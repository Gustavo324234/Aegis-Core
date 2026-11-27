const {
  withAndroidManifest,
  withDangerousMod,
  withMainApplication,
  withMainActivity,
} = require('expo/config-plugins');
const fs = require('fs');
const path = require('path');

/**
 * Expo Config Plugin (EPIC 63 / CORE-415):
 * Configures Aegis as an Android Default Digital Assistant (`ACTION_ASSIST` / `VOICE_COMMAND`),
 * adds a `FOREGROUND_SERVICE_MICROPHONE` always-on service (`AegisVoiceForegroundService`),
 * and exposes `AegisAssistantModule` for 16kHz PCM breath-aware capture & background wake listening.
 */
function withAegisAssistant(config) {
  const isPlayStoreBuild =
    process.env.PLAY_STORE_BUILD === '1' || process.env.PLAY_STORE_BUILD === 'true';

  config = withAndroidManifest(config, (cfg) => {
    const manifest = cfg.modResults;
    const appPkg = manifest.manifest;

    const requiredPerms = [
      'android.permission.RECORD_AUDIO',
      'android.permission.FOREGROUND_SERVICE',
      'android.permission.FOREGROUND_SERVICE_MICROPHONE',
      'android.permission.WAKE_LOCK',
      'android.permission.POST_NOTIFICATIONS',
    ];
    if (!isPlayStoreBuild) {
      requiredPerms.push('android.permission.REQUEST_INSTALL_PACKAGES');
    }
    appPkg['uses-permission'] = appPkg['uses-permission'] || [];
    for (const perm of requiredPerms) {
      const exists = appPkg['uses-permission'].some(
        (item) => item.$ && item.$['android:name'] === perm
      );
      if (!exists) {
        appPkg['uses-permission'].push({ $: { 'android:name': perm } });
      }
    }

    const application = appPkg.application && appPkg.application[0];
    if (application) {
      application.activity = application.activity || [];
      const mainActivity = application.activity.find(
        (act) => act.$ && act.$['android:name'] === '.MainActivity'
      );
      if (mainActivity) {
        mainActivity['intent-filter'] = mainActivity['intent-filter'] || [];
        const ensureAssistAction = (actionName) => {
          const hasFilter = mainActivity['intent-filter'].some((f) =>
            (f.action || []).some((a) => a.$ && a.$['android:name'] === actionName)
          );
          if (!hasFilter) {
            mainActivity['intent-filter'].push({
              action: [{ $: { 'android:name': actionName } }],
              category: [{ $: { 'android:name': 'android.intent.category.DEFAULT' } }],
            });
          }
        };
        ensureAssistAction('android.intent.action.ASSIST');
        ensureAssistAction('android.intent.action.VOICE_COMMAND');
      }

      application.service = application.service || [];
      const serviceName = '.voice.AegisVoiceForegroundService';
      if (!application.service.some((s) => s.$ && s.$['android:name'] === serviceName)) {
        application.service.push({
          $: {
            'android:name': serviceName,
            'android:enabled': 'true',
            'android:exported': 'false',
            'android:foregroundServiceType': 'microphone',
          },
        });
      }

      if (!isPlayStoreBuild) {
        application.provider = application.provider || [];
        const providerAuth = '${applicationId}.aegis.fileprovider';
        const hasProvider = application.provider.some(
          (p) => p.$ && p.$['android:authorities'] === providerAuth
        );
        if (!hasProvider) {
          application.provider.push({
            $: {
              'android:name': 'androidx.core.content.FileProvider',
              'android:authorities': providerAuth,
              'android:exported': 'false',
              'android:grantUriPermissions': 'true',
            },
            'meta-data': [
              {
                $: {
                  'android:name': 'android.support.FILE_PROVIDER_PATHS',
                  'android:resource': '@xml/aegis_updater_paths',
                },
              },
            ],
          });
        }
      }
    }
    return cfg;
  });

  config = withMainActivity(config, (cfg) => {
    let contents = cfg.modResults.contents;
    if (!contents.includes('AegisAssistantModule')) {
      contents = contents.replace(
        'import android.os.Bundle',
        'import android.content.Intent\nimport android.os.Bundle\nimport io.aegis.app.voice.AegisAssistantModule'
      );
      if (!contents.includes('override fun onNewIntent')) {
        contents = contents.replace(
          'override fun onCreate(savedInstanceState: Bundle?) {',
          `override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    AegisAssistantModule.handleLaunchIntent(intent)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    AegisAssistantModule.handleLaunchIntent(intent)`
        );
      }
    }
    cfg.modResults.contents = contents;
    return cfg;
  });

  config = withMainApplication(config, (cfg) => {
    let contents = cfg.modResults.contents;
    const importLine = 'import io.aegis.app.voice.AegisAssistantPackage';
    if (!contents.includes(importLine)) {
      contents = contents.replace(
        'import expo.modules.ApplicationLifecycleDispatcher',
        `import expo.modules.ApplicationLifecycleDispatcher\n${importLine}`
      );
      contents = contents.replace(
        'packages.apply {',
        'packages.apply {\n              add(AegisAssistantPackage())'
      );
    }
    cfg.modResults.contents = contents;
    return cfg;
  });

  config = withDangerousMod(config, [
    'android',
    async (cfg) => {
      const voiceDir = path.join(
        cfg.modRequest.platformProjectRoot,
        'app/src/main/java/io/aegis/app/voice'
      );
      fs.mkdirSync(voiceDir, { recursive: true });

      fs.writeFileSync(path.join(voiceDir, 'AegisAssistantPackage.kt'), KOTLIN_PACKAGE);
      fs.writeFileSync(
        path.join(voiceDir, 'AegisAssistantModule.kt'),
        KOTLIN_MODULE.replace(
          '__AEGIS_SUPPORTS_IN_APP_UPDATE__',
          isPlayStoreBuild ? 'false' : 'true'
        )
      );
      fs.writeFileSync(path.join(voiceDir, 'AegisVoiceForegroundService.kt'), KOTLIN_SERVICE);

      if (!isPlayStoreBuild) {
        const xmlDir = path.join(
          cfg.modRequest.platformProjectRoot,
          'app/src/main/res/xml'
        );
        fs.mkdirSync(xmlDir, { recursive: true });
        fs.writeFileSync(
          path.join(xmlDir, 'aegis_updater_paths.xml'),
          UPDATER_PATHS_XML
        );
      }
      return cfg;
    },
  ]);

  return config;
}

const KOTLIN_PACKAGE = `package io.aegis.app.voice

import com.facebook.react.ReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.uimanager.ViewManager

class AegisAssistantPackage : ReactPackage {
  override fun createNativeModules(reactContext: ReactApplicationContext): List<NativeModule> =
    listOf(AegisAssistantModule(reactContext))

  override fun createViewManagers(reactContext: ReactApplicationContext): List<ViewManager<*, *>> =
    emptyList()
}
`;

const KOTLIN_MODULE = `package io.aegis.app.voice

import android.content.Intent
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.provider.Settings
import android.util.Base64
import androidx.core.content.FileProvider
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlin.concurrent.thread
import kotlin.math.sqrt

class AegisAssistantModule(private val reactCtx: ReactApplicationContext) :
  ReactContextBaseJavaModule(reactCtx) {

  override fun getName(): String = "AegisAssistantModule"

  companion object {
    @Volatile
    var pendingAssistTrigger: Boolean = false
    @Volatile
    var pendingVoiceCommand: String? = null

    fun handleLaunchIntent(intent: Intent?) {
      val cmd = intent?.getStringExtra("aegis_voice_command")?.trim()
      if (!cmd.isNullOrEmpty()) {
        pendingVoiceCommand = cmd
        intent.removeExtra("aegis_voice_command")
      }
      val action = intent?.action ?: return
      if (action == Intent.ACTION_ASSIST || action == Intent.ACTION_VOICE_COMMAND) {
        pendingAssistTrigger = true
      }
    }
  }

  @ReactMethod
  fun openAssistantSettings(promise: Promise) {
    try {
      val intent = Intent(Settings.ACTION_VOICE_INPUT_SETTINGS).apply {
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      }
      reactCtx.startActivity(intent)
      promise.resolve(true)
    } catch (e: Exception) {
      promise.reject("ERR_SETTINGS", e)
    }
  }

  @ReactMethod
  fun consumeAssistLaunchTrigger(promise: Promise) {
    val triggered = pendingAssistTrigger
    pendingAssistTrigger = false
    promise.resolve(triggered)
  }

  @ReactMethod
  fun consumePendingVoiceCommand(promise: Promise) {
    val cmd = pendingVoiceCommand
    pendingVoiceCommand = null
    promise.resolve(cmd)
  }

  @ReactMethod
  fun startContinuousListening(
    serverUrl: String,
    tenantId: String,
    sessionKey: String,
    agentName: String,
    hangoverMs: Int,
    promise: Promise
  ) {
    try {
      val intent = Intent(reactCtx, AegisVoiceForegroundService::class.java).apply {
        action = AegisVoiceForegroundService.ACTION_START
        putExtra("serverUrl", serverUrl)
        putExtra("tenantId", tenantId)
        putExtra("sessionKey", sessionKey)
        putExtra("agentName", agentName)
        putExtra("hangoverMs", hangoverMs)
      }
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        reactCtx.startForegroundService(intent)
      } else {
        reactCtx.startService(intent)
      }
      promise.resolve(true)
    } catch (e: Exception) {
      promise.reject("ERR_START_SERVICE", e)
    }
  }

  @ReactMethod
  fun stopContinuousListening(promise: Promise) {
    try {
      val intent = Intent(reactCtx, AegisVoiceForegroundService::class.java).apply {
        action = AegisVoiceForegroundService.ACTION_STOP
      }
      reactCtx.startService(intent)
      promise.resolve(true)
    } catch (e: Exception) {
      promise.reject("ERR_STOP_SERVICE", e)
    }
  }

  @ReactMethod
  fun isContinuousListening(promise: Promise) {
    promise.resolve(AegisVoiceForegroundService.isRunning)
  }

  @ReactMethod
  fun getAppVersionInfo(promise: Promise) {
    try {
      val pm = reactCtx.packageManager
      val pInfo = pm.getPackageInfo(reactCtx.packageName, 0)
      val code = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        pInfo.longVersionCode.toDouble()
      } else {
        @Suppress("DEPRECATION")
        pInfo.versionCode.toDouble()
      }
      val map = Arguments.createMap().apply {
        putString("versionName", pInfo.versionName ?: "1.0.0")
        putDouble("versionCode", code)
        putBoolean("supportsInAppUpdate", __AEGIS_SUPPORTS_IN_APP_UPDATE__)
      }
      promise.resolve(map)
    } catch (e: Exception) {
      promise.reject("ERR_VERSION", e)
    }
  }

  @ReactMethod
  fun downloadAndInstallApk(apkUrl: String, expectedSha256: String?, promise: Promise) {
    if (!__AEGIS_SUPPORTS_IN_APP_UPDATE__) {
      promise.reject("ERR_PLAY_BUILD", "Play Store build uses Google Play updates.")
      return
    }
    thread(start = true) {
      try {
        var currentUrl = apkUrl
        var conn: HttpURLConnection? = null
        for (hop in 0..5) {
          val candidate = (URL(currentUrl).openConnection() as HttpURLConnection).apply {
            instanceFollowRedirects = true
            connectTimeout = 15000
            readTimeout = 60000
            requestMethod = "GET"
          }
          val status = candidate.responseCode
          if (status in 300..399) {
            val loc = candidate.getHeaderField("Location")
            candidate.disconnect()
            if (loc.isNullOrBlank()) {
              promise.reject("ERR_REDIRECT", "Redirect without Location header")
              return@thread
            }
            currentUrl = loc
            continue
          }
          conn = candidate
          break
        }

        val activeConn = conn ?: run {
          promise.reject("ERR_DOWNLOAD", "Too many redirects")
          return@thread
        }
        if (activeConn.responseCode !in 200..299) {
          val code = activeConn.responseCode
          activeConn.disconnect()
          promise.reject("ERR_HTTP", "Failed to download APK: HTTP " + code)
          return@thread
        }

        val updatesDir = File(reactCtx.cacheDir, "updates").apply { mkdirs() }
        val apkFile = File(updatesDir, "aegis-update.apk")
        val digest = MessageDigest.getInstance("SHA-256")
        val buf = ByteArray(16384)

        activeConn.inputStream.use { input ->
          FileOutputStream(apkFile).use { output ->
            while (true) {
              val n = input.read(buf)
              if (n <= 0) break
              digest.update(buf, 0, n)
              output.write(buf, 0, n)
            }
          }
        }
        activeConn.disconnect()

        val actualSha = digest.digest().joinToString("") { "%02x".format(it) }
        val expectedClean = expectedSha256?.trim()?.lowercase() ?: ""
        if (expectedClean.isNotEmpty() && actualSha != expectedClean) {
          apkFile.delete()
          promise.reject(
            "ERR_CHECKSUM_MISMATCH",
            "SHA-256 mismatch: expected " + expectedClean + ", got " + actualSha
          )
          return@thread
        }

        val contentUri = FileProvider.getUriForFile(
          reactCtx,
          reactCtx.packageName + ".aegis.fileprovider",
          apkFile
        )
        val installIntent = Intent(Intent.ACTION_VIEW).apply {
          setDataAndType(contentUri, "application/vnd.android.package-archive")
          addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        reactCtx.startActivity(installIntent)
        promise.resolve(actualSha)
      } catch (e: Exception) {
        promise.reject("ERR_INSTALL_APK", e)
      }
    }
  }

  /**
   * Captures a single 16 kHz PCM16 mono utterance with adaptive VAD and breath-tolerant
   * hangover (hangoverMs), or a fixed sample when fixedDurationMs > 0 for voice enrollment.
   */
  @ReactMethod
  fun recordPcm16Utterance(
    hangoverMs: Int,
    fixedDurationMs: Int,
    promise: Promise
  ) {
    thread(start = true) {
      val sampleRate = 16000
      val frameSamples = 480 // 30ms
      val minBuf = AudioRecord.getMinBufferSize(
        sampleRate,
        AudioFormat.CHANNEL_IN_MONO,
        AudioFormat.ENCODING_PCM_16BIT
      ).coerceAtLeast(frameSamples * 4)

      val recorder = AudioRecord(
        MediaRecorder.AudioSource.VOICE_RECOGNITION,
        sampleRate,
        AudioFormat.CHANNEL_IN_MONO,
        AudioFormat.ENCODING_PCM_16BIT,
        minBuf
      )
      if (recorder.state != AudioRecord.STATE_INITIALIZED) {
        promise.reject("ERR_MIC_INIT", "AudioRecord initialization failed")
        return@thread
      }

      try {
        val pcmBuf = ShortArray(frameSamples)
        val preRoll = ArrayDeque<ByteArray>()
        val maxPreFrames = 17 // ~500ms
        val out = ByteArrayOutputStream()

        var hasSpoken = fixedDurationMs > 0
        var noiseEma = 0.008
        var speechStreak = 0
        val startedAt = System.currentTimeMillis()
        var lastSpeechAt = startedAt

        recorder.startRecording()

        while (true) {
          val read = recorder.read(pcmBuf, 0, frameSamples)
          if (read <= 0) continue

          val bytes = ByteArray(read * 2)
          var sq = 0.0
          for (i in 0 until read) {
            val s = pcmBuf[i]
            val norm = s / 32768.0
            sq += norm * norm
            bytes[2 * i] = (s.toInt() and 0xff).toByte()
            bytes[2 * i + 1] = ((s.toInt() shr 8) and 0xff).toByte()
          }
          val rms = sqrt(sq / read.coerceAtLeast(1))
          val now = System.currentTimeMillis()

          if (fixedDurationMs > 0) {
            out.write(bytes)
            if (now - startedAt >= fixedDurationMs) break
            continue
          }

          val threshold = maxOf(0.015, noiseEma * 2.4)
          if (!hasSpoken) {
            if (preRoll.size >= maxPreFrames) preRoll.removeFirst()
            preRoll.addLast(bytes)
            if (rms > threshold) {
              speechStreak++
              if (speechStreak >= 2) {
                hasSpoken = true
                lastSpeechAt = now
                for (chunk in preRoll) out.write(chunk)
              }
            } else {
              speechStreak = 0
              noiseEma = noiseEma * 0.95 + rms * 0.05
              if (now - startedAt >= 10000) break
            }
          } else {
            out.write(bytes)
            if (rms > threshold * 0.85) {
              lastSpeechAt = now
            }
            if (now - lastSpeechAt >= hangoverMs || now - startedAt >= 40000) {
              break
            }
          }
        }

        val raw = out.toByteArray()
        val result = Arguments.createMap().apply {
          putString("pcmB64", Base64.encodeToString(raw, Base64.NO_WRAP))
          putInt("sampleRate", sampleRate)
          putBoolean("hadSpeech", hasSpoken && raw.size >= 3200)
        }
        promise.resolve(result)
      } catch (e: Exception) {
        promise.reject("ERR_CAPTURE", e)
      } finally {
        try {
          recorder.stop()
        } catch (_: Exception) {
        }
        recorder.release()
      }
    }
  }
}
`;

const KOTLIN_SERVICE = `package io.aegis.app.voice

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.os.IBinder
import android.speech.tts.TextToSpeech
import android.util.Base64
import androidx.core.app.NotificationCompat
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread
import kotlin.math.sqrt

class AegisVoiceForegroundService : Service(), TextToSpeech.OnInitListener {

  companion object {
    const val ACTION_START = "io.aegis.app.voice.START"
    const val ACTION_STOP = "io.aegis.app.voice.STOP"
    private const val CHANNEL_ID = "aegis_voice_always_on"
    private const val NOTIF_ID = 6301
    @Volatile
    var isRunning: Boolean = false
  }

  private val loopActive = AtomicBoolean(false)
  private val ttsBusy = AtomicBoolean(false)
  private var tts: TextToSpeech? = null
  private var ttsReady = false

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    tts = TextToSpeech(applicationContext, this)
  }

  override fun onInit(status: Int) {
    if (status == TextToSpeech.SUCCESS) {
      tts?.language = Locale("es", "AR")
      ttsReady = true
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == ACTION_STOP) {
      loopActive.set(false)
      isRunning = false
      stopForeground(STOP_FOREGROUND_REMOVE)
      stopSelf()
      return START_NOT_STICKY
    }

    val serverUrl = intent?.getStringExtra("serverUrl") ?: return START_NOT_STICKY
    val tenantId = intent.getStringExtra("tenantId") ?: return START_NOT_STICKY
    val sessionKey = intent.getStringExtra("sessionKey") ?: return START_NOT_STICKY
    val agentName = intent.getStringExtra("agentName") ?: "Aegis"
    val hangoverMs = intent.getIntExtra("hangoverMs", 2400)

    ensureNotificationChannel()
    val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
    val pi = PendingIntent.getActivity(
      this,
      0,
      launchIntent,
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
    )
    val notif = NotificationCompat.Builder(this, CHANNEL_ID)
      .setContentTitle("Aegis atento")
      .setContentText("Decí \\"$agentName escuchame\\" en cualquier momento")
      .setSmallIcon(android.R.drawable.ic_btn_speak_now)
      .setOngoing(true)
      .setContentIntent(pi)
      .build()

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      startForeground(
        NOTIF_ID,
        notif,
        ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
      )
    } else {
      startForeground(NOTIF_ID, notif)
    }

    if (loopActive.compareAndSet(false, true)) {
      isRunning = true
      thread(start = true) {
        runListenLoop(serverUrl.trimEnd('/'), tenantId, sessionKey, hangoverMs)
      }
    }
    return START_STICKY
  }

  private fun runListenLoop(
    baseUrl: String,
    tenantId: String,
    sessionKey: String,
    hangoverMs: Int
  ) {
    val sampleRate = 16000
    val frameSamples = 480
    val minBuf = AudioRecord.getMinBufferSize(
      sampleRate,
      AudioFormat.CHANNEL_IN_MONO,
      AudioFormat.ENCODING_PCM_16BIT
    ).coerceAtLeast(frameSamples * 4)

    val recorder = AudioRecord(
      MediaRecorder.AudioSource.VOICE_RECOGNITION,
      sampleRate,
      AudioFormat.CHANNEL_IN_MONO,
      AudioFormat.ENCODING_PCM_16BIT,
      minBuf
    )
    if (recorder.state != AudioRecord.STATE_INITIALIZED) {
      loopActive.set(false)
      isRunning = false
      stopSelf()
      return
    }

    val pcmBuf = ShortArray(frameSamples)
    val preRoll = ArrayDeque<ByteArray>()
    val speechBuf = ByteArrayOutputStream()
    var inSpeech = false
    var noiseEma = 0.008
    var streak = 0
    var speechStartAt = 0L
    var lastSpeechAt = 0L
    var followupUntil = 0L

    recorder.startRecording()
    try {
      while (loopActive.get()) {
        val read = recorder.read(pcmBuf, 0, frameSamples)
        if (read <= 0) continue
        if (ttsBusy.get()) {
          inSpeech = false
          speechBuf.reset()
          continue
        }

        val bytes = ByteArray(read * 2)
        var sq = 0.0
        for (i in 0 until read) {
          val s = pcmBuf[i]
          val n = s / 32768.0
          sq += n * n
          bytes[2 * i] = (s.toInt() and 0xff).toByte()
          bytes[2 * i + 1] = ((s.toInt() shr 8) and 0xff).toByte()
        }
        val rms = sqrt(sq / read.coerceAtLeast(1))
        val threshold = maxOf(0.015, noiseEma * 2.5)
        val now = System.currentTimeMillis()
        val inFollowup = now < followupUntil

        if (!inSpeech) {
          if (preRoll.size >= 17) preRoll.removeFirst()
          preRoll.addLast(bytes)
          if (rms > threshold) {
            streak++
            if (streak >= 2) {
              inSpeech = true
              speechStartAt = now
              lastSpeechAt = now
              speechBuf.reset()
              for (c in preRoll) speechBuf.write(c)
            }
          } else {
            streak = 0
            noiseEma = noiseEma * 0.95 + rms * 0.05
          }
        } else {
          speechBuf.write(bytes)
          if (rms > threshold * 0.85) lastSpeechAt = now
          val activeHangover = if (inFollowup) hangoverMs else 1400
          if (now - lastSpeechAt >= activeHangover || now - speechStartAt >= 30000) {
            val payload = speechBuf.toByteArray()
            inSpeech = false
            streak = 0
            speechBuf.reset()
            if (payload.size >= 14400) {
              val mode = if (inFollowup) "followup" else "wakeword"
              val res = postUtterance(baseUrl, tenantId, sessionKey, payload, mode)
              if (res != null && res.optBoolean("accepted", false)) {
                if (res.optBoolean("wake_only", false)) {
                  followupUntil = System.currentTimeMillis() + 10000L
                } else {
                  val cmd = res.optString("command", "").trim()
                  if (cmd.isNotEmpty()) {
                    // Launch main app intent with prompt or notify backend chat
                    openOrDispatchCommand(baseUrl, tenantId, sessionKey, cmd)
                    followupUntil = System.currentTimeMillis() + 8000L
                  }
                }
              }
            }
          }
        }
      }
    } finally {
      try {
        recorder.stop()
      } catch (_: Exception) {
      }
      recorder.release()
    }
  }

  private fun postUtterance(
    baseUrl: String,
    tenantId: String,
    sessionKey: String,
    pcm: ByteArray,
    mode: String
  ): JSONObject? {
    return try {
      val conn = (URL("$baseUrl/api/siren/utterance").openConnection() as HttpURLConnection).apply {
        requestMethod = "POST"
        connectTimeout = 8000
        readTimeout = 20000
        doOutput = true
        setRequestProperty("Content-Type", "application/json")
        setRequestProperty("x-citadel-tenant", tenantId)
        setRequestProperty("x-citadel-key", sessionKey)
      }
      val body = JSONObject().apply {
        put("pcm_b64", Base64.encodeToString(pcm, Base64.NO_WRAP))
        put("sample_rate", 16000)
        put("mode", mode)
        put("lang", "es-AR")
      }.toString()
      conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
      if (conn.responseCode in 200..299) {
        JSONObject(conn.inputStream.bufferedReader().readText())
      } else null
    } catch (_: Exception) {
      null
    }
  }

  private fun openOrDispatchCommand(
    baseUrl: String,
    tenantId: String,
    sessionKey: String,
    command: String
  ) {
    // Open MainActivity carrying pending command or use TTS confirmation
    val launch = packageManager.getLaunchIntentForPackage(packageName)?.apply {
      addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
      putExtra("aegis_voice_command", command)
    }
    if (ttsReady) {
      ttsBusy.set(true)
      tts?.speak("Entendido.", TextToSpeech.QUEUE_FLUSH, null, "aegis_ack")
      Thread.sleep(650)
      ttsBusy.set(false)
    }
    if (launch != null) {
      startActivity(launch)
    }
  }

  private fun ensureNotificationChannel() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val nm = getSystemService(NotificationManager::class.java)
      val channel = NotificationChannel(
        CHANNEL_ID,
        "Aegis Escucha Continua",
        NotificationManager.IMPORTANCE_LOW
      )
      nm.createNotificationChannel(channel)
    }
  }

  override fun onDestroy() {
    loopActive.set(false)
    isRunning = false
    tts?.stop()
    tts?.shutdown()
    super.onDestroy()
  }
}
`;

const UPDATER_PATHS_XML = `<?xml version="1.0" encoding="utf-8"?>
<paths xmlns:android="http://schemas.android.com/apk/res/android">
  <cache-path name="aegis_updates" path="updates/" />
</paths>
`;

module.exports = withAegisAssistant;
