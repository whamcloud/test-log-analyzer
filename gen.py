import random
import os
import sys

def generate_constant_log_file(filename, target_size_mb):
    levels = ["WARN", "INFO", "ERROR"]
    
    constant_timestamp = "2025-01-01T12:00:00Z"
    constant_domain = "auth"
    constant_message = "invalid token"
    
    malformed_templates = [
        "2025-01-01T12:00:00Z INVALID_SEPARATOR auth message without pipes",
        "|WARN|database|missing timestamp entirely",
        "2025-01-02T15:30:22Z|INFO||empty module section",
        "CORRUPT_BYTES_\x00\x01\x02\x03_CRASH_DUMP_CORRUPTION_LINE",
        "2025-01-03T09:11:00Z|DEBUG|network|truncated mid-sentence due to ",
        "--- SYSTEM REBOOT LOG BUFFER PURGED ---",
        "2025-01-04!!!10:20:30Z|ERROR|api|malformed ISO timestamp delimiter",
        "2025-01-05T11:12:13Z|INVALID_LEVEL|storage|this log level does not exist",
    ]

    target_bytes = target_size_mb * 1024 * 1024
    current_bytes = 0
    
    print(f"Generating ~{target_size_mb} MB log file with constant fields at '{filename}'...")
    
    with open(filename, "w", encoding="utf-8", buffering=10*1024*1024) as f:
        while current_bytes < target_bytes:
            # 2% chance to inject an invalid/malformed log line
            if random.random() < 0.02:
                log_line = random.choice(malformed_templates) + "\n"
            else:
                level = random.choice(levels)
                log_line = f"{constant_timestamp}|{level}|{constant_domain}|{constant_message}\n"
            
            f.write(log_line)
            current_bytes += len(log_line.encode("utf-8"))

    actual_size_mb = os.path.getsize(filename) / (1024 * 1024)
    print(f"Success! Generated file size: {actual_size_mb:.2f} MB")

if __name__ == "__main__":
    target_size = 250
    if len(sys.argv) > 1:
        try:
            target_size = int(sys.argv[1])
        except ValueError:
            print("Error: Target size must be an integer.")
            sys.exit(1)
    print(f"Generating log file with size: {target_size} MB")
    generate_constant_log_file("test1.dat",target_size)
