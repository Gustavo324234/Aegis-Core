import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';

const PORT = Number(process.env.WA_BRIDGE_PORT || 8100);
const HOST = '127.0.0.1';
const SECRET = process.env.WA_BRIDGE_SECRET || '';
const BASE_DATA_DIR = path.resolve(
  process.env.WA_DATA_DIR || path.join(process.cwd(), '.sessions')
);

const MAX_CHATS = 300;
const MAX_CONTACTS = 2000;
const MAX_MESSAGES_PER_CHAT = 60;
const MIN_BRIDGE_SEND_GAP_MS = 1500;

let depsPromise = null;
function loadWhatsAppDeps() {
  if (!depsPromise) {
    depsPromise = (async () => {
      const [{ Boom }, baileysMod, pinoMod, qrMod] = await Promise.all([
        import('@hapi/boom'),
        import('@whiskeysockets/baileys'),
        import('pino'),
        import('qrcode'),
      ]);
      const makeWASocket = baileysMod.default ?? baileysMod.makeWASocket;
      const pino = pinoMod.default ?? pinoMod;
      const QRCode = qrMod.default ?? qrMod;
      const logger = pino({ level: process.env.WA_LOG_LEVEL || 'silent' });
      return {
        Boom,
        makeWASocket,
        DisconnectReason: baileysMod.DisconnectReason,
        fetchLatestBaileysVersion: baileysMod.fetchLatestBaileysVersion,
        useMultiFileAuthState: baileysMod.useMultiFileAuthState,
        QRCode,
        logger,
      };
    })().catch((err) => {
      depsPromise = null;
      throw err;
    });
  }
  return depsPromise;
}

const sessions = new Map();

function sanitizeTenant(raw) {
  const cleaned = String(raw || 'root').trim();
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(cleaned)) {
    throw new Error('Invalid tenant identifier');
  }
  return cleaned;
}

function constantTimeBearerCheck(authHeader) {
  if (!SECRET) return false;
  if (!authHeader || !authHeader.startsWith('Bearer ')) return false;
  const provided = authHeader.slice('Bearer '.length).trim();
  const a = Buffer.from(provided, 'utf8');
  const b = Buffer.from(SECRET, 'utf8');
  if (a.length !== b.length) return false;
  return crypto.timingSafeEqual(a, b);
}

async function ensurePrivateDir(dirPath) {
  await fs.mkdir(dirPath, { recursive: true, mode: 0o700 });
  if (process.platform !== 'win32') {
    await fs.chmod(dirPath, 0o700).catch(() => {});
  }
}

async function tightenSecretFilePerms(filePath) {
  if (process.platform !== 'win32') {
    await fs.chmod(filePath, 0o600).catch(() => {});
  }
}

function normalizePhone(jidOrPhone) {
  const str = String(jidOrPhone || '');
  const left = str.includes('@') ? str.split('@')[0] : str;
  const base = left.includes(':') ? left.split(':')[0] : left;
  return base.replace(/[^0-9]/g, '');
}

function normalizeSearchText(str) {
  return String(str || '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .trim()
    .toLowerCase();
}

function extractRealPhoneFromJid(jid, obj) {
  const candidates = [obj?.jid, obj?.phoneNumber, jid];
  for (const c of candidates) {
    if (!c) continue;
    const s = String(c);
    if (s.endsWith('@lid') || s.endsWith('@g.us')) continue;
    const digits = normalizePhone(s);
    if (digits.length >= 6) return digits;
  }
  return '';
}

function unwrapMessage(msg) {
  let m = msg;
  while (m) {
    if (m.ephemeralMessage?.message) {
      m = m.ephemeralMessage.message;
      continue;
    }
    if (m.viewOnceMessage?.message) {
      m = m.viewOnceMessage.message;
      continue;
    }
    if (m.viewOnceMessageV2?.message) {
      m = m.viewOnceMessageV2.message;
      continue;
    }
    if (m.documentWithCaptionMessage?.message) {
      m = m.documentWithCaptionMessage.message;
      continue;
    }
    break;
  }
  return m || {};
}

function extractMessageText(rawMessage) {
  const m = unwrapMessage(rawMessage);
  if (!m) return '';
  if (typeof m.conversation === 'string') return m.conversation;
  if (m.extendedTextMessage?.text) return m.extendedTextMessage.text;
  if (m.imageMessage) {
    return m.imageMessage.caption ? `[Imagen] ${m.imageMessage.caption}` : '[Imagen]';
  }
  if (m.videoMessage) {
    return m.videoMessage.caption ? `[Video] ${m.videoMessage.caption}` : '[Video]';
  }
  if (m.audioMessage) {
    return m.audioMessage.ptt ? '[Audio de voz]' : '[Audio]';
  }
  if (m.documentMessage) {
    const fname = m.documentMessage.fileName || 'archivo';
    return m.documentMessage.caption
      ? `[Documento: ${fname}] ${m.documentMessage.caption}`
      : `[Documento: ${fname}]`;
  }
  if (m.stickerMessage) return '[Sticker]';
  if (m.locationMessage) return '[Ubicación]';
  if (m.contactMessage?.displayName) return `[Contacto: ${m.contactMessage.displayName}]`;
  if (m.buttonsResponseMessage?.selectedDisplayText) {
    return m.buttonsResponseMessage.selectedDisplayText;
  }
  if (m.listResponseMessage?.title) return m.listResponseMessage.title;
  return '';
}

function toTimestampSec(ts) {
  if (!ts) return Math.floor(Date.now() / 1000);
  if (typeof ts === 'number') return ts > 1e12 ? Math.floor(ts / 1000) : ts;
  if (typeof ts === 'object' && typeof ts.low === 'number') return ts.low;
  const parsed = Number(ts);
  return Number.isFinite(parsed) && parsed > 0
    ? parsed > 1e12
      ? Math.floor(parsed / 1000)
      : parsed
    : Math.floor(Date.now() / 1000);
}

class TenantWhatsAppSession {
  constructor(tenantId) {
    this.tenantId = tenantId;
    this.baseDir = path.join(BASE_DATA_DIR, tenantId);
    this.authDir = path.join(this.baseDir, 'auth');
    this.storePath = path.join(this.baseDir, 'store.json');
    this.sock = null;
    this.status = 'disconnected'; // disconnected | connecting | qr_ready | connected | logged_out
    this.qrText = null;
    this.qrDataUrl = null;
    this.qrUpdatedAt = null;
    this.lastError = null;
    this.userJid = null;
    this.userName = null;
    this.phone = null;
    this.reconnectTimer = null;
    this.reconnectAttempt = 0;
    this.manualStop = false;
    this.connectingPromise = null;
    this.lastSendAtMs = 0;
    this.chats = new Map(); // jid -> { jid, name, phone, isGroup, unreadCount, lastMessageTs, lastMessage }
    this.contacts = new Map(); // jid -> { jid, name, notify, verifiedName, phone }
    this.messages = new Map(); // chatJid -> Array<{ id, fromMe, senderJid, senderName, text, timestamp }>
    this.saveTimer = null;
    this.storeLoaded = false;
  }

  async ensureLoaded() {
    if (this.storeLoaded) return;
    this.storeLoaded = true;
    await ensurePrivateDir(this.baseDir);
    await ensurePrivateDir(this.authDir);
    await tightenSecretFilePerms(this.storePath);
    await tightenSecretFilePerms(path.join(this.authDir, 'creds.json'));
    try {
      const raw = await fs.readFile(this.storePath, 'utf8');
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed.chats)) {
        for (const c of parsed.chats) {
          if (c?.jid) this.chats.set(c.jid, c);
        }
      }
      if (Array.isArray(parsed.contacts)) {
        for (const c of parsed.contacts) {
          if (c?.jid) this.contacts.set(c.jid, c);
        }
      }
      if (parsed.messages && typeof parsed.messages === 'object') {
        for (const [jid, msgs] of Object.entries(parsed.messages)) {
          if (Array.isArray(msgs)) {
            this.messages.set(jid, msgs.slice(-MAX_MESSAGES_PER_CHAT));
          }
        }
      }
    } catch {
      // Fresh store
    }
  }

  scheduleSave() {
    if (this.saveTimer) return;
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      this.persistStore().catch(() => {});
    }, 1000);
  }

  async persistStore() {
    await ensurePrivateDir(this.baseDir);
    const sortedChats = [...this.chats.values()]
      .sort((a, b) => (b.lastMessageTs || 0) - (a.lastMessageTs || 0))
      .slice(0, MAX_CHATS);
    const keepJids = new Set(sortedChats.map((c) => c.jid));
    const messagesObj = {};
    for (const [jid, list] of this.messages.entries()) {
      if (keepJids.has(jid) || list.length > 0) {
        messagesObj[jid] = list.slice(-MAX_MESSAGES_PER_CHAT);
      }
    }
    const sortedContacts = [...this.contacts.values()]
      .sort((a, b) => {
        const aScore = (a.name ? 4 : 0) + (a.verifiedName ? 2 : 0) + (a.notify ? 1 : 0);
        const bScore = (b.name ? 4 : 0) + (b.verifiedName ? 2 : 0) + (b.notify ? 1 : 0);
        return bScore - aScore;
      })
      .slice(0, MAX_CONTACTS);
    const payload = {
      chats: sortedChats,
      contacts: sortedContacts,
      messages: messagesObj,
    };
    const tmpPath = `${this.storePath}.tmp`;
    await fs.writeFile(tmpPath, JSON.stringify(payload), { encoding: 'utf8', mode: 0o600 });
    await fs.rename(tmpPath, this.storePath);
    await tightenSecretFilePerms(this.storePath);
  }

  async hasSavedCreds() {
    try {
      await fs.access(path.join(this.authDir, 'creds.json'));
      return true;
    } catch {
      return false;
    }
  }

  upsertContact(c) {
    if (!c?.id) return;
    const jid = c.id;
    const prev = this.contacts.get(jid) || { jid, phone: '' };
    const lid = c.lid || (jid.endsWith('@lid') ? jid : prev.lid || '');
    const phoneJid =
      c.jid && !String(c.jid).endsWith('@lid')
        ? c.jid
        : jid.endsWith('@s.whatsapp.net')
          ? jid
          : prev.phoneJid || '';
    const phone = extractRealPhoneFromJid(jid, c) || prev.phone || '';
    const next = {
      jid,
      lid,
      phoneJid: phoneJid || (phone ? `${phone}@s.whatsapp.net` : ''),
      phone,
      name: c.name || prev.name || '',
      notify: c.notify || prev.notify || '',
      verifiedName: c.verifiedName || prev.verifiedName || '',
    };
    this.contacts.set(jid, next);
    const bestName = next.name || next.verifiedName || next.notify;
    for (const targetJid of [jid, next.phoneJid].filter(Boolean)) {
      const chat = this.chats.get(targetJid);
      if (chat) {
        if (bestName && (!chat.name || chat.name === chat.phone || chat.name === chat.jid)) {
          chat.name = bestName;
        }
        if (phone && !chat.phone) {
          chat.phone = phone;
        }
      }
    }
  }

  displayNameFor(jid, fallbackPushName = '') {
    const contact = this.contacts.get(jid);
    const chat = this.chats.get(jid);
    const candidate =
      chat?.name ||
      contact?.name ||
      contact?.verifiedName ||
      contact?.notify ||
      fallbackPushName ||
      normalizePhone(jid) ||
      jid;
    return String(candidate);
  }

  recordMessage(msg) {
    const key = msg?.key;
    if (!key?.remoteJid) return;
    const chatJid = key.remoteJid;
    if (chatJid === 'status@broadcast') return;

    const text = extractMessageText(msg.message);
    if (!text) return;

    const timestamp = toTimestampSec(msg.messageTimestamp);
    const fromMe = Boolean(key.fromMe);
    const senderJid = fromMe
      ? this.userJid || chatJid
      : key.participant || chatJid;
    const pushName = msg.pushName || '';
    if (!fromMe && pushName) {
      this.upsertContact({ id: senderJid, notify: pushName });
    }
    const senderName = fromMe
      ? this.userName || 'Yo'
      : this.displayNameFor(senderJid, pushName);

    const existingChat = this.chats.get(chatJid) || {
      jid: chatJid,
      name: this.displayNameFor(chatJid, !chatJid.endsWith('@g.us') && !fromMe ? pushName : ''),
      phone: chatJid.endsWith('@g.us') ? '' : normalizePhone(chatJid),
      isGroup: chatJid.endsWith('@g.us'),
      unreadCount: 0,
      lastMessageTs: 0,
      lastMessage: '',
    };

    if (!existingChat.isGroup && !fromMe && pushName && existingChat.name === existingChat.phone) {
      existingChat.name = pushName;
    }

    if (timestamp >= (existingChat.lastMessageTs || 0)) {
      existingChat.lastMessageTs = timestamp;
      existingChat.lastMessage = text.slice(0, 200);
    }
    if (!fromMe) {
      existingChat.unreadCount = (existingChat.unreadCount || 0) + 1;
    }
    this.chats.set(chatJid, existingChat);

    const list = this.messages.get(chatJid) || [];
    const msgId = key.id || `${timestamp}-${list.length}`;
    if (!list.some((m) => m.id === msgId)) {
      list.push({
        id: msgId,
        fromMe,
        senderJid,
        senderName,
        text,
        timestamp,
      });
      list.sort((a, b) => a.timestamp - b.timestamp);
      if (list.length > MAX_MESSAGES_PER_CHAT) {
        list.splice(0, list.length - MAX_MESSAGES_PER_CHAT);
      }
      this.messages.set(chatJid, list);
    }

    this.scheduleSave();
  }

  getStatusSnapshot() {
    return {
      tenant_id: this.tenantId,
      status: this.status,
      connected: this.status === 'connected',
      phone: this.phone,
      name: this.userName,
      jid: this.userJid,
      qr_data_url: this.qrDataUrl,
      qr_text: this.qrText,
      qr_updated_at: this.qrUpdatedAt,
      last_error: this.lastError,
      chats_count: this.chats.size,
    };
  }

  async connect() {
    if (this.status === 'connected' && this.sock) {
      return this.getStatusSnapshot();
    }

    this.manualStop = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (!this.connectingPromise) {
      this.status = 'connecting';
      this.lastError = null;
      this.connectingPromise = (async () => {
        try {
          await this.ensureLoaded();
          const {
            Boom,
            makeWASocket,
            DisconnectReason,
            fetchLatestBaileysVersion,
            useMultiFileAuthState,
            QRCode,
            logger,
          } = await loadWhatsAppDeps();

          const { state, saveCreds } = await useMultiFileAuthState(this.authDir);
          let version;
          try {
            const latest = await fetchLatestBaileysVersion();
            version = latest.version;
          } catch {
            version = undefined;
          }

          if (this.sock) {
            try {
              this.sock.end(undefined);
            } catch {}
            this.sock = null;
          }

          const sock = makeWASocket({
            version,
            auth: state,
            logger,
            printQRInTerminal: false,
            browser: ['Aegis OS', 'Chrome', '126.0.0'],
            markOnlineOnConnect: false,
            syncFullHistory: false,
          });
          this.sock = sock;

          sock.ev.on('creds.update', async () => {
            await saveCreds();
            await tightenSecretFilePerms(path.join(this.authDir, 'creds.json'));
          });

          sock.ev.on('contacts.upsert', (list) => {
            for (const c of list || []) this.upsertContact(c);
            this.scheduleSave();
          });
          sock.ev.on('contacts.update', (list) => {
            for (const c of list || []) this.upsertContact(c);
            this.scheduleSave();
          });
          sock.ev.on('chats.upsert', (list) => {
            for (const c of list || []) {
              if (!c?.id || c.id === 'status@broadcast') continue;
              const prev = this.chats.get(c.id) || {
                jid: c.id,
                name: this.displayNameFor(c.id),
                phone: c.id.endsWith('@g.us') ? '' : normalizePhone(c.id),
                isGroup: c.id.endsWith('@g.us'),
                unreadCount: 0,
                lastMessageTs: 0,
                lastMessage: '',
              };
              this.chats.set(c.id, {
                ...prev,
                name: c.name || prev.name,
                unreadCount: typeof c.unreadCount === 'number' ? c.unreadCount : prev.unreadCount,
                lastMessageTs: c.conversationTimestamp
                  ? Math.max(prev.lastMessageTs || 0, toTimestampSec(c.conversationTimestamp))
                  : prev.lastMessageTs,
              });
            }
            this.scheduleSave();
          });
          sock.ev.on('messaging-history.set', ({ chats, contacts, messages }) => {
            for (const c of contacts || []) this.upsertContact(c);
            for (const c of chats || []) {
              if (!c?.id || c.id === 'status@broadcast') continue;
              const isGroup = c.id.endsWith('@g.us');
              const rawTs = c.conversationTimestamp || c.lastMsgTimestamp || c.t || 0;
              const parsedTs = rawTs ? toTimestampSec(rawTs) : 0;
              const prev = this.chats.get(c.id) || {
                jid: c.id,
                name: this.displayNameFor(c.id),
                phone: isGroup ? '' : extractRealPhoneFromJid(c.id, c),
                isGroup,
                unreadCount: 0,
                lastMessageTs: 0,
                lastMessage: '',
              };
              const nextName =
                c.name ||
                (prev.name && prev.name !== c.id ? prev.name : this.displayNameFor(c.id));
              this.chats.set(c.id, {
                ...prev,
                name: nextName || prev.name,
                phone: prev.phone || (isGroup ? '' : extractRealPhoneFromJid(c.id, c)),
                lastMessageTs: Math.max(prev.lastMessageTs || 0, parsedTs),
                unreadCount: typeof c.unreadCount === 'number' ? c.unreadCount : prev.unreadCount,
              });
            }
            for (const m of messages || []) this.recordMessage(m);
            this.scheduleSave();
          });

          sock.ev.on('messages.upsert', ({ messages }) => {
            for (const m of messages || []) this.recordMessage(m);
          });

          sock.ev.on('connection.update', async (update) => {
            if (this.sock !== sock) return;
            const { connection, lastDisconnect, qr } = update;

            if (qr) {
              this.qrText = qr;
              try {
                this.qrDataUrl = await QRCode.toDataURL(qr, { width: 280, margin: 2 });
              } catch {
                this.qrDataUrl = null;
              }
              this.qrUpdatedAt = Date.now();
              this.status = 'qr_ready';
            }

            if (connection === 'open') {
              this.reconnectAttempt = 0;
              this.qrText = null;
              this.qrDataUrl = null;
              this.lastError = null;
              this.status = 'connected';
              this.userJid = sock.user?.id || null;
              this.userName = sock.user?.name || null;
              this.phone = normalizePhone(sock.user?.id || '') || null;
              try {
                const groups = await sock.groupFetchAllParticipating();
                for (const g of Object.values(groups || {})) {
                  const prev = this.chats.get(g.id) || {
                    jid: g.id,
                    name: g.subject || g.id,
                    phone: '',
                    isGroup: true,
                    unreadCount: 0,
                    lastMessageTs: 0,
                    lastMessage: '',
                  };
                  prev.name = g.subject || prev.name;
                  this.chats.set(g.id, prev);
                }
                this.scheduleSave();
              } catch {
                // Non-fatal group metadata fetch
              }
            }

            if (connection === 'close') {
              const errObj = lastDisconnect?.error;
              const statusCode =
                errObj instanceof Boom
                  ? errObj.output?.statusCode
                  : errObj?.output?.statusCode;
              const loggedOut = statusCode === DisconnectReason.loggedOut || statusCode === 401;

              if (this.manualStop) {
                this.status = 'disconnected';
                this.qrText = null;
                this.qrDataUrl = null;
                return;
              }

              if (loggedOut) {
                this.status = 'logged_out';
                this.qrText = null;
                this.qrDataUrl = null;
                this.lastError = 'Sesión cerrada desde WhatsApp en tu teléfono.';
                this.sock = null;
                await fs.rm(this.authDir, { recursive: true, force: true }).catch(() => {});
                await ensurePrivateDir(this.authDir).catch(() => {});
                return;
              }

              this.status = 'connecting';
              this.lastError = errObj?.message || `Reconectando (código ${statusCode || 'desconocido'})`;
              this.reconnectAttempt += 1;
              const delayMs = Math.min(1500 * Math.pow(1.6, Math.min(this.reconnectAttempt, 5)), 15000);
              if (!this.reconnectTimer) {
                this.reconnectTimer = setTimeout(() => {
                  this.reconnectTimer = null;
                  this.connect().catch(() => {});
                }, delayMs);
              }
            }
          });
        } catch (err) {
          this.status = 'disconnected';
          this.lastError = err?.message || String(err);
        } finally {
          this.connectingPromise = null;
        }
      })();
    }

    // Wait briefly (up to 3.5s) so the HTTP caller returns promptly while cold ESM imports finish in background.
    const deadline = Date.now() + 3500;
    while (Date.now() < deadline) {
      if (this.status === 'qr_ready' || this.status === 'connected' || this.status === 'logged_out') {
        break;
      }
      if (!this.connectingPromise && this.status === 'disconnected') {
        break;
      }
      await new Promise((r) => setTimeout(r, 150));
    }

    return this.getStatusSnapshot();
  }

  async disconnect(logout = false) {
    this.manualStop = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    const sock = this.sock;
    this.sock = null;
    this.qrText = null;
    this.qrDataUrl = null;
    this.status = logout ? 'logged_out' : 'disconnected';

    if (sock) {
      try {
        if (logout) {
          await sock.logout();
        } else {
          sock.end(undefined);
        }
      } catch {
        try {
          sock.end(undefined);
        } catch {}
      }
    }

    if (logout) {
      this.userJid = null;
      this.userName = null;
      this.phone = null;
      await fs.rm(this.authDir, { recursive: true, force: true }).catch(() => {});
      await ensurePrivateDir(this.authDir).catch(() => {});
    }

    await this.persistStore().catch(() => {});
    return this.getStatusSnapshot();
  }

  buildMergedDirectory() {
    const merged = new Map();

    const canonicalKeyForChat = (c) => {
      if (c.isGroup) return c.jid;
      const phone = c.phone || extractRealPhoneFromJid(c.jid, c);
      return phone || c.jid;
    };

    for (const c of this.chats.values()) {
      const key = canonicalKeyForChat(c);
      const existing = merged.get(key);
      const phone = c.isGroup ? '' : c.phone || extractRealPhoneFromJid(c.jid, c);
      const name = c.name || this.displayNameFor(c.jid, phone ? `+${phone}` : c.jid);
      if (!existing) {
        merged.set(key, {
          jid: phone && c.jid.endsWith('@lid') ? `${phone}@s.whatsapp.net` : c.jid,
          name,
          phone,
          isGroup: Boolean(c.isGroup),
          unreadCount: c.unreadCount || 0,
          lastMessageTs: c.lastMessageTs || 0,
          lastMessage: c.lastMessage || '',
        });
      } else {
        if ((c.lastMessageTs || 0) > (existing.lastMessageTs || 0)) {
          existing.lastMessageTs = c.lastMessageTs || 0;
          existing.lastMessage = c.lastMessage || existing.lastMessage;
        }
        if (
          name &&
          (!existing.name || existing.name === existing.jid || existing.name === `+${existing.phone}`)
        ) {
          existing.name = name;
        }
        if (!existing.phone && phone) existing.phone = phone;
        if (existing.jid.endsWith('@lid') && !c.jid.endsWith('@lid')) {
          existing.jid = c.jid;
        }
        existing.unreadCount = Math.max(existing.unreadCount || 0, c.unreadCount || 0);
      }
    }

    for (const contact of this.contacts.values()) {
      const bestName = contact.name || contact.verifiedName || contact.notify || '';
      const phone = contact.phone || extractRealPhoneFromJid(contact.jid, contact);
      const bestJid =
        contact.phoneJid || (phone ? `${phone}@s.whatsapp.net` : '') || contact.jid;
      if (!bestName && !phone) continue;
      const key = phone || bestJid;
      const existing = merged.get(key);
      if (existing) {
        if (
          bestName &&
          (!existing.name ||
            existing.name === existing.jid ||
            existing.name === existing.phone ||
            existing.name === `+${existing.phone}`)
        ) {
          existing.name = bestName;
        }
        const extraNames = [contact.name, contact.verifiedName, contact.notify].filter(Boolean);
        if (extraNames.length) {
          existing._aliases = [...new Set([...(existing._aliases || []), ...extraNames])];
        }
        if (!existing.phone && phone) existing.phone = phone;
        if (existing.jid.endsWith('@lid') && bestJid && !bestJid.endsWith('@lid')) {
          existing.jid = bestJid;
        }
      } else {
        merged.set(key, {
          jid: bestJid,
          name: bestName || `+${phone}`,
          phone,
          isGroup: false,
          unreadCount: 0,
          lastMessageTs: 0,
          lastMessage: '',
          _aliases: [contact.name, contact.verifiedName, contact.notify].filter(Boolean),
        });
      }
    }

    return [...merged.values()];
  }

  listChats(search = '', limit = 20) {
    const qNorm = normalizeSearchText(search);
    const digits = normalizePhone(search);
    const items = this.buildMergedDirectory()
      .filter((c) => {
        if (!qNorm) return true;
        const names = [c.name, ...(c._aliases || [])].filter(Boolean);
        if (names.some((n) => normalizeSearchText(n).includes(qNorm))) return true;
        if (normalizeSearchText(c.jid).includes(qNorm)) return true;
        if (digits && (c.phone || '').includes(digits)) return true;
        return false;
      })
      .sort((a, b) => {
        if (qNorm) {
          const bestRank = (entry) => {
            const names = [entry.name, ...(entry._aliases || [])]
              .map(normalizeSearchText)
              .filter(Boolean);
            if (names.some((n) => n === qNorm)) return 2;
            if (names.some((n) => n.startsWith(qNorm))) return 1;
            return 0;
          };
          const rankDiff = bestRank(b) - bestRank(a);
          if (rankDiff !== 0) return rankDiff;
          if (Boolean(a.isGroup) !== Boolean(b.isGroup)) return a.isGroup ? 1 : -1;
        }
        const tsDiff = (b.lastMessageTs || 0) - (a.lastMessageTs || 0);
        if (tsDiff !== 0) return tsDiff;
        if (Boolean(a.isGroup) !== Boolean(b.isGroup)) return a.isGroup ? 1 : -1;
        return (a.name || '').localeCompare(b.name || '');
      })
      .slice(0, Math.min(Math.max(Number(limit) || 20, 1), 100))
      .map(({ _aliases, ...pub }) => pub);
    return items;
  }

  listContacts(search = '', limit = 50) {
    const qNorm = normalizeSearchText(search);
    const digits = normalizePhone(search);
    return this.buildMergedDirectory()
      .filter((c) => !c.isGroup)
      .filter((c) => {
        if (!qNorm) return true;
        const names = [c.name, ...(c._aliases || [])].filter(Boolean);
        if (names.some((n) => normalizeSearchText(n).includes(qNorm))) return true;
        if (digits && (c.phone || '').includes(digits)) return true;
        return false;
      })
      .sort((a, b) => {
        if (qNorm) {
          const aPrefix = [a.name, ...(a._aliases || [])]
            .some((n) => normalizeSearchText(n).startsWith(qNorm))
            ? 1
            : 0;
          const bPrefix = [b.name, ...(b._aliases || [])]
            .some((n) => normalizeSearchText(n).startsWith(qNorm))
            ? 1
            : 0;
          if (aPrefix !== bPrefix) return bPrefix - aPrefix;
        }
        const tsDiff = (b.lastMessageTs || 0) - (a.lastMessageTs || 0);
        if (tsDiff !== 0) return tsDiff;
        return (a.name || '').localeCompare(b.name || '');
      })
      .slice(0, Math.min(Math.max(Number(limit) || 50, 1), 150))
      .map(({ _aliases, ...pub }) => pub);
  }

  async resolveTarget(target) {
    const raw = String(target || '').trim();
    if (!raw) {
      throw new Error('Destinatario vacío');
    }

    if (raw.includes('@')) {
      const chat = this.chats.get(raw);
      return {
        jid: raw,
        label: chat?.name || this.displayNameFor(raw),
      };
    }

    const qNorm = normalizeSearchText(raw);
    const digits = normalizePhone(raw);

    // If user explicitly provided a phone number (7+ digits, mostly numeric)
    const strippedRaw = raw.replace(/[\s+()-]/g, '');
    if (digits.length >= 7 && /^\d+$/.test(strippedRaw)) {
      const dirMatch = this.buildMergedDirectory().find((d) => !d.isGroup && d.phone === digits);
      if (this.sock && this.status === 'connected') {
        try {
          const check = await this.sock.onWhatsApp(digits);
          if (Array.isArray(check) && check[0]?.exists && check[0]?.jid) {
            const jid = check[0].jid;
            return { jid, label: dirMatch?.name || this.displayNameFor(jid, `+${digits}`) };
          }
          // Argentina convenience fallback (+54 -> +549) if not found directly
          if (digits.startsWith('54') && !digits.startsWith('549')) {
            const arCheck = await this.sock.onWhatsApp(`549${digits.slice(2)}`);
            if (Array.isArray(arCheck) && arCheck[0]?.exists && arCheck[0]?.jid) {
              const jid = arCheck[0].jid;
              return { jid, label: dirMatch?.name || this.displayNameFor(jid, `+${digits}`) };
            }
          }
        } catch {
          // fallback to standard JID
        }
      }
      const fallbackJid = dirMatch?.jid || `${digits}@s.whatsapp.net`;
      return { jid: fallbackJid, label: dirMatch?.name || this.displayNameFor(fallbackJid, `+${digits}`) };
    }

    // Search by accent-insensitive name across merged chats and contacts
    const matches = [];
    for (const entry of this.buildMergedDirectory()) {
      const allNormNames = [entry.name, ...(entry._aliases || [])]
        .map(normalizeSearchText)
        .filter(Boolean);
      const matchingNames = allNormNames.filter((n) => n.includes(qNorm));
      if (matchingNames.length === 0) continue;
      matches.push({
        jid: entry.jid,
        name: entry.name || entry.phone || entry.jid,
        phone: entry.phone || normalizePhone(entry.jid),
        isGroup: Boolean(entry.isGroup),
        exact: matchingNames.some((n) => n === qNorm),
        prefix: matchingNames.some((n) => n.startsWith(qNorm)),
        ts: entry.lastMessageTs || 0,
      });
    }

    matches.sort((a, b) => {
      if (a.exact !== b.exact) return a.exact ? -1 : 1;
      if (a.prefix !== b.prefix) return a.prefix ? -1 : 1;
      if (a.isGroup !== b.isGroup) return a.isGroup ? 1 : -1;
      return b.ts - a.ts;
    });

    if (matches.length === 0) {
      throw new Error(
        `No encontré ningún contacto o chat que coincida con '${raw}'. Podés pasarme el número con código de país (o guardarlo como contacto/alias) para ubicarlo al instante.`
      );
    }
    const exactMatches = matches.filter((m) => m.exact);
    if (exactMatches.length === 1) {
      return { jid: exactMatches[0].jid, label: exactMatches[0].name };
    }
    if (matches.length > 1) {
      const err = new Error(`Hay múltiples contactos que coinciden con '${raw}'`);
      err.code = 'AMBIGUOUS_RECIPIENT';
      err.candidates = matches.slice(0, 5).map((m) => ({
        jid: m.jid,
        name: m.name,
        phone: m.phone || normalizePhone(m.jid),
      }));
      throw err;
    }

    return { jid: matches[0].jid, label: matches[0].name };
  }

  async readMessages(chatQuery, limit = 20) {
    const resolved = await this.resolveTarget(chatQuery);
    const msgs = (this.messages.get(resolved.jid) || []).slice(
      -Math.min(Math.max(Number(limit) || 20, 1), MAX_MESSAGES_PER_CHAT)
    );
    const chat = this.chats.get(resolved.jid);
    if (chat) {
      chat.unreadCount = 0;
      this.scheduleSave();
    }
    return {
      chat: {
        jid: resolved.jid,
        name: resolved.label,
        phone: normalizePhone(resolved.jid),
      },
      count: msgs.length,
      messages: msgs,
    };
  }

  async sendMessage(to, text) {
    if (!this.sock || this.status !== 'connected') {
      throw new Error('WhatsApp no está conectado. Escaneá el QR en Settings → Cuentas Conectadas.');
    }
    const cleanText = String(text || '').trim();
    if (!cleanText) {
      throw new Error('El texto del mensaje no puede estar vacío.');
    }

    const elapsed = Date.now() - this.lastSendAtMs;
    if (elapsed < MIN_BRIDGE_SEND_GAP_MS) {
      await new Promise((r) => setTimeout(r, MIN_BRIDGE_SEND_GAP_MS - elapsed));
    }

    const resolved = await this.resolveTarget(to);
    const sent = await this.sock.sendMessage(resolved.jid, { text: cleanText });
    this.lastSendAtMs = Date.now();
    if (sent) {
      this.recordMessage({
        key: sent.key || { remoteJid: resolved.jid, fromMe: true, id: `local-${Date.now()}` },
        message: sent.message || { conversation: cleanText },
        messageTimestamp: Math.floor(Date.now() / 1000),
      });
    }
    return {
      status: 'sent',
      message_id: sent?.key?.id || null,
      to_jid: resolved.jid,
      to_label: resolved.label,
      text: cleanText,
    };
  }
}

function getSession(tenantId) {
  let s = sessions.get(tenantId);
  if (!s) {
    s = new TenantWhatsAppSession(tenantId);
    sessions.set(tenantId, s);
  }
  return s;
}

async function readJsonBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > 1024 * 1024) {
      throw new Error('Payload too large');
    }
    chunks.push(chunk);
  }
  const raw = Buffer.concat(chunks).toString('utf8').trim();
  return raw ? JSON.parse(raw) : {};
}

function sendJson(res, statusCode, data) {
  const body = JSON.stringify(data);
  res.writeHead(statusCode, {
    'Content-Type': 'application/json; charset=utf-8',
    'Content-Length': Buffer.byteLength(body),
  });
  res.end(body);
}

const server = http.createServer(async (req, res) => {
  try {
    const parsedUrl = new URL(req.url || '/', `http://${HOST}:${PORT}`);
    const pathname = parsedUrl.pathname;

    if (req.method === 'GET' && pathname === '/health') {
      return sendJson(res, 200, { ok: true, service: 'aegis-whatsapp-bridge' });
    }

    if (!constantTimeBearerCheck(req.headers.authorization)) {
      return sendJson(res, 401, { error: 'unauthorized' });
    }

    if (req.method === 'GET' && pathname === '/status') {
      const tenantId = sanitizeTenant(parsedUrl.searchParams.get('tenant'));
      const session = getSession(tenantId);
      await session.ensureLoaded();
      return sendJson(res, 200, session.getStatusSnapshot());
    }

    if (req.method === 'POST' && pathname === '/connect') {
      const body = await readJsonBody(req);
      const tenantId = sanitizeTenant(body.tenant || parsedUrl.searchParams.get('tenant'));
      const session = getSession(tenantId);
      const snapshot = await session.connect();
      return sendJson(res, 200, snapshot);
    }

    if (req.method === 'POST' && pathname === '/disconnect') {
      const body = await readJsonBody(req);
      const tenantId = sanitizeTenant(body.tenant || parsedUrl.searchParams.get('tenant'));
      const logout = Boolean(body.logout);
      const session = getSession(tenantId);
      const snapshot = await session.disconnect(logout);
      return sendJson(res, 200, snapshot);
    }

    if (req.method === 'GET' && pathname === '/chats') {
      const tenantId = sanitizeTenant(parsedUrl.searchParams.get('tenant'));
      const search = parsedUrl.searchParams.get('search') || '';
      const limit = Number(parsedUrl.searchParams.get('limit') || 20);
      const session = getSession(tenantId);
      await session.ensureLoaded();
      const chats = session.listChats(search, limit);
      return sendJson(res, 200, {
        status: session.status,
        connected: session.status === 'connected',
        count: chats.length,
        total_chats: session.chats.size,
        total_contacts: session.contacts.size,
        chats,
      });
    }

    if (req.method === 'GET' && pathname === '/contacts') {
      const tenantId = sanitizeTenant(parsedUrl.searchParams.get('tenant'));
      const search = parsedUrl.searchParams.get('search') || '';
      const limit = Number(parsedUrl.searchParams.get('limit') || 50);
      const session = getSession(tenantId);
      await session.ensureLoaded();
      const contacts = session.listContacts(search, limit);
      return sendJson(res, 200, {
        status: session.status,
        connected: session.status === 'connected',
        count: contacts.length,
        total_contacts: session.contacts.size,
        contacts,
      });
    }

    if (req.method === 'GET' && pathname === '/messages') {
      const tenantId = sanitizeTenant(parsedUrl.searchParams.get('tenant'));
      const chat = parsedUrl.searchParams.get('chat') || '';
      const limit = Number(parsedUrl.searchParams.get('limit') || 20);
      const session = getSession(tenantId);
      await session.ensureLoaded();
      const data = await session.readMessages(chat, limit);
      return sendJson(res, 200, {
        status: session.status,
        connected: session.status === 'connected',
        ...data,
      });
    }

    if (req.method === 'POST' && pathname === '/resolve') {
      const body = await readJsonBody(req);
      const tenantId = sanitizeTenant(body.tenant || parsedUrl.searchParams.get('tenant'));
      const session = getSession(tenantId);
      await session.ensureLoaded();
      const resolved = await session.resolveTarget(body.to || body.target || '');
      return sendJson(res, 200, resolved);
    }

    if (req.method === 'POST' && pathname === '/send') {
      const body = await readJsonBody(req);
      const tenantId = sanitizeTenant(body.tenant || parsedUrl.searchParams.get('tenant'));
      const session = getSession(tenantId);
      const result = await session.sendMessage(body.to, body.text);
      return sendJson(res, 200, result);
    }

    return sendJson(res, 404, { error: 'not_found' });
  } catch (err) {
    const status = err?.code === 'AMBIGUOUS_RECIPIENT' ? 409 : 400;
    return sendJson(res, status, {
      error: err?.code || 'bridge_error',
      message: err?.message || String(err),
      candidates: err?.candidates || undefined,
    });
  }
});

async function autoWarmSavedTenants() {
  try {
    await ensurePrivateDir(BASE_DATA_DIR);
    const entries = await fs.readdir(BASE_DATA_DIR, { withFileTypes: true });
    for (const ent of entries) {
      if (!ent.isDirectory()) continue;
      try {
        const tenantId = sanitizeTenant(ent.name);
        const session = getSession(tenantId);
        if (await session.hasSavedCreds()) {
          session.connect().catch(() => {});
        }
      } catch {}
    }
  } catch {}
}

server.listen(PORT, HOST, () => {
  if (!SECRET) {
    console.error(
      '[SECURITY] WA_BRIDGE_SECRET is not configured; all authenticated WhatsApp bridge endpoints will fail closed with 401.'
    );
  }
  console.log(`Aegis WhatsApp Bridge listening on http://${HOST}:${PORT}`);
  loadWhatsAppDeps().catch(() => {});
  autoWarmSavedTenants();
});

async function shutdown() {
  for (const s of sessions.values()) {
    await s.disconnect(false).catch(() => {});
  }
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(0), 2000);
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
