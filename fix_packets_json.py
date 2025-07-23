import json

with open("crates/valence_generated/extracted/packets.json", "r", encoding="utf-8") as f:
    data = json.load(f)

for packet in data:
    if "phase" in packet and "state" not in packet:
        packet["state"] = packet["phase"]

with open("crates/valence_generated/extracted/packets.json", "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)

