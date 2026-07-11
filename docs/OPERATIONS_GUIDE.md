# Aegis OS — Operations & KeyPool Recovery Guide (OPS-001)

This operations guide outlines the architecture of the Aegis Core **KeyPool (API keys registry)**, describes how keys are persisted in the SQLCipher database, and provides standard procedures for restoring, auditing, and re-registering API keys after a service recreation or database corruption event.

---

## 🔑 1. KeyPool Architecture & Persistence

Aegis OS utilizes an encrypted SQLite database via SQLCipher (`aegis.db`) to persist system configurations, agent enclaves, and user credentials. 

To maintain a unified storage model, **API keys are persisted as synthetic Process Control Blocks (PCBs)** with special Process Identifiers (PIDs):
* **Global Keys:** Saved with the PID prefix `keypool:global:{key_id}`.
* **Tenant Personal Keys:** Saved with the PID prefix `keypool:tenant:{tenant_id}:{key_id}`.

The raw JSON payload of the `ApiKeyEntry` is stored directly inside the PCB's L1 memory slot (`l1_instruction`).

### How Keys Are Loaded on Startup
When the Aegis service (`ank-server`) boots:
1. The `KeyPool` component executes `load()`.
2. It requests all persisted PCBs from the database.
3. It filters PIDs starting with `keypool:` and deserializes their L1 slots into the active memory `KeyPool` structure.
4. Deserialization warnings/errors are captured and logged as `tracing::error!` under `CORE-213`.

---

## 🚧 2. The API Key Reset Issue (OPS-001)

### The Problem
During certain update or migration scenarios:
1. **Root Key Rotations:** If the `AEGIS_ROOT_KEY` in `aegis.env` is regenerated or modified, SQLCipher will fail to decrypt the existing `aegis.db`. The database integrity check fails, and the installer automatically quarantines the old database (`aegis.db.bak_<timestamp>`) and initializes a clean, empty database.
2. **Clean Installs / Recreations:** Moving the Aegis service to a new shared host or clearing active Docker volumes resets the sqlite db, causing the active enclaves and keys to be wiped out.

Under these circumstances, the active Multi-Agent loop (`run_agent_loop`) will fail immediately with errors indicating **"No active key/provider configured"** because the loaded KeyPool is empty.

---

## 🛠️ 3. Operational Recovery Procedures

Follow these step-by-step SRE procedures to query, dump, and restore API keys on Aegis OS.

### Step 3.1 — Verify KeyPool Status via CLI

To verify if Aegis currently has active keys loaded:
```bash
# On Linux / Windows CLI:
aegis diag
```
Inspect **Section [3] CONFIGURATION** and **[4] PORTS** to ensure the service is running, then examine logs for KeyPool deserialization errors.

Alternatively, you can run a diagnostic shell probe:
```bash
# Using gRPC status command to inspect loaded models and active processes
aegis status
```

---

### Step 3.2 — Workaround: Automated Key Backup & Restore Script

For automated SRE operations, save the utility script below as `re_register_keys.py` anywhere on the host to bulk-register fallback API keys using the REST API directly.

Create the recovery payload file `keys_backup.json`:
```json
[
  {
    "key_id": "global-openrouter-primary",
    "provider": "openrouter",
    "api_key": "sk-or-v1-xxxxxxxxxxxxxxxxx",
    "label": "Primary OpenRouter Key",
    "is_active": true,
    "is_free_tier": false
  },
  {
    "key_id": "global-openai-fallback",
    "provider": "openai",
    "api_key": "sk-proj-xxxxxxxxxxxxxxxxx",
    "label": "Fallback OpenAI Key",
    "is_active": true,
    "is_free_tier": false
  }
]
```

Run the re-registration Python script to inject the keys into the clean instance:
```python
import json
import http.client
import sys

def restore_keys(backup_file="keys_backup.json", host="localhost", port=8000, root_key=""):
    try:
        with open(backup_file, "r") as f:
            keys = json.load(f)
    except Exception as e:
        print(f"Error loading backup file: {e}")
        sys.exit(1)

    headers = {
        "Content-Type": "application/json",
        "x-aegis-root-key": root_key
    }

    conn = http.client.HTTPConnection(host, port)
    
    for key in keys:
        payload = json.dumps(key)
        conn.request("POST", "/api/router/keys/global", body=payload, headers=headers)
        resp = conn.getresponse()
        data = resp.read().decode()
        
        if resp.status in (200, 201):
            print(f"✅ Key '{key['key_id']}' for provider '{key['provider']}' successfully registered.")
        else:
            print(f"❌ Failed to register key '{key['key_id']}': {resp.status} - {data}")
            
    conn.close()

if __name__ == "__main__":
    # Supply your actual active AEGIS_ROOT_KEY from aegis.env here
    ACTIVE_ROOT_KEY = "your_hex_root_key_from_aegis_env"
    restore_keys(root_key=ACTIVE_ROOT_KEY, port=8000)
```

---

### Step 3.3 — Restoring Personal Tenant Keys

To register or restore **Tenant-specific personal keys**, the tenant can securely run these via the front-end **Settings → Engine (Motor)** tab using the verified personal key flow.

If an administrator needs to bulk-restore tenant-scoped keys programmatically, they can POST to the tenant endpoint:
```http
POST /api/router/keys/tenant
x-citadel-tenant: <tenant_id>
x-citadel-key: <tenant_session_key>
Content-Type: application/json

{
  "key_id": "tenant-groq-key",
  "provider": "groq",
  "api_key": "gsk_xxxx",
  "label": "My Groq Key",
  "is_active": true,
  "is_free_tier": true
}
```

---

## 🚨 4. SRE Hardening Checklist

To prevent future database corruption and key loss:
1. **Regular Backups:** Back up `/var/lib/aegis/aegis.db` (Linux) or `C:\ProgramData\Aegis\aegis.db` (Windows) daily using a standard SCM registry backup script.
2. **Protect root key:** Ensure `AEGIS_ROOT_KEY` is backed up securely. If the root key is lost, the SQLCipher database **cannot be decrypted** under any circumstances.
3. **Graceful Shutdowns:** Always stop the Aegis service (`aegis stop`) prior to doing host server restarts or service updates to prevent SQLite write conflicts and lockups.
