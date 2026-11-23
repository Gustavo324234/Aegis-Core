import os
import shutil
import sys
import tarfile
import zipfile

def safe_extract(archive_path: str, bin_dir: str):
    os.makedirs(bin_dir, exist_ok=True)
    targets = {"ffmpeg", "ffprobe", "ffmpeg.exe", "ffprobe.exe"}
    found = []

    if archive_path.lower().endswith(".zip"):
        with zipfile.ZipFile(archive_path, "r") as zf:
            for member in zf.infolist():
                norm = member.filename.replace("\\", "/")
                if ".." in norm.split("/") or norm.startswith("/"):
                    continue
                base = os.path.basename(norm)
                if base in targets:
                    dest = os.path.join(bin_dir, base)
                    with zf.open(member, "r") as src, open(dest, "wb") as dst:
                        shutil.copyfileobj(src, dst)
                    if os.name != "nt":
                        os.chmod(dest, 0o755)
                    found.append(base)
    else:
        with tarfile.open(archive_path, "r:*") as tf:
            for member in tf.getmembers():
                if not member.isfile():
                    continue
                norm = member.name.replace("\\", "/")
                if ".." in norm.split("/") or norm.startswith("/"):
                    continue
                base = os.path.basename(norm)
                if base in targets:
                    src = tf.extractfile(member)
                    if src is not None:
                        dest = os.path.join(bin_dir, base)
                        with src, open(dest, "wb") as dst:
                            shutil.copyfileobj(src, dst)
                        if os.name != "nt":
                            os.chmod(dest, 0o755)
                        found.append(base)

    if not any(f.startswith("ffmpeg") for f in found):
        raise SystemExit(f"Archive did not contain ffmpeg binary (found: {found})")
    print("Extracted:", ",".join(sorted(set(found))))

if __name__ == "__main__":
    safe_extract(sys.argv[1], sys.argv[2])
