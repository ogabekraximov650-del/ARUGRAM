"""grammers sessiyasidan (JSON) DrKLO Telegram (Android) sessiya fayllarini yaratadi.

Kirish : session.json  ({"home_dc","dcs":[{id,ipv4,ipv6,auth_key}],"user":{id,phone,first_name}})
Chiqish: <papka>/tgnet.dat        — tgnet konfiguratsiyasi (DC va avtorizatsiya kaliti), `files/` papkasiga
         <papka>/userconfing.xml  — hisob (TLRPC.TL_user, self), `shared_prefs/` papkasiga

Format DrKLO manba kodidagi `ConnectionsManager::saveConfigInternal`, `Datacenter::serializeToStream` va `UserConfig.loadConfig` dan
(o'qib o'zimiz yozdik). Ochiq matnli kalit faqat shu jarayon xotirasida va chiqish fayllarida bo'ladi — chiqishni shifrlang.

Ishlatish: python to_drklo.py session.json <chiqish_papkasi>
"""
import base64, hashlib, json, os, struct, sys, time

CONFIG_VERSION_NET = 5      # ConnectionsManager::configVersion
CONFIG_VERSION_DC = 13      # Datacenter::configVersion
DEFAULT_IPV4 = {1: "149.154.175.53", 2: "149.154.167.51", 3: "149.154.175.100", 4: "149.154.167.91", 5: "91.108.56.130"}


class Buf:
    def __init__(self):
        self.b = bytearray()

    def i32(self, v):
        self.b += struct.pack("<i", v)

    def u32(self, v):
        self.b += struct.pack("<I", v)

    def i64(self, v):
        self.b += struct.pack("<q", v)

    def boolean(self, v):
        self.u32(0x997275B5 if v else 0xBC799737)

    def raw(self, data):
        self.b += data

    def string(self, data):
        if isinstance(data, str):
            data = data.encode()
        n = len(data)
        if n <= 253:
            self.b.append(n)
            sl = 1
        else:
            self.b.append(254)
            self.b += bytes([n & 255, (n >> 8) & 255, (n >> 16) & 255])
            sl = 4
        self.b += data
        pad = (n + sl) % 4
        if pad:
            self.b += b"\0" * (4 - pad)


def key_bytes(v):
    if isinstance(v, list):
        return bytes(v)
    if isinstance(v, str):
        try:
            return bytes.fromhex(v) if len(v) == 512 else base64.b64decode(v)
        except Exception:
            return base64.b64decode(v)
    raise ValueError("auth_key formati noma'lum")


def addr_of(dc):
    v = dc.get("ipv4")
    if isinstance(v, str):
        return v.split(":")[0], int(v.split(":")[1]) if ":" in v else 443
    if isinstance(v, list) and len(v) == 4:
        return ".".join(str(x) for x in v), 443
    if isinstance(v, dict):
        v = next((x for x in v.values() if isinstance(x, (list, str, int))), None)
        return addr_of({"id": dc["id"], "ipv4": v})
    if isinstance(v, int):
        return ".".join(str((v >> s) & 255) for s in (24, 16, 8, 0)), 443
    return DEFAULT_IPV4.get(dc["id"], DEFAULT_IPV4[2]), 443


def main(src, out):
    s = json.load(open(src))
    home = s["home_dc"]
    dc = next(d for d in s["dcs"] if d["id"] == home)
    key = key_bytes(dc["auth_key"])
    assert len(key) == 256, f"auth_key uzunligi {len(key)}"
    key_id = struct.unpack("<q", hashlib.sha1(key).digest()[12:20])[0]
    ip, port = addr_of(dc)

    d = Buf()
    d.i32(CONFIG_VERSION_DC)
    d.i32(home)
    d.i32(0)      # lastInitVersion
    d.i32(0)      # lastInitMediaVersion
    for arr in range(4):
        if arr == 0:
            d.i32(1)
            d.string(ip); d.i32(port); d.i32(0); d.string("")
        else:
            d.i32(0)
    d.boolean(False)           # isCdnDatacenter
    d.i32(256); d.raw(key)     # authKeyPerm
    d.i64(key_id)
    d.i32(0); d.i64(0)         # authKeyTemp
    d.i32(0); d.i64(0)         # authKeyMediaTemp
    d.i32(1)                   # authorized
    d.i32(0)                   # serverSalts
    d.i32(0)                   # mediaServerSalts

    c = Buf()
    c.i32(CONFIG_VERSION_NET)
    c.boolean(False)           # testBackend
    c.boolean(False)           # clientBlocked
    c.string("")               # lastInitSystemLangcode
    c.boolean(True)
    c.i32(home)
    c.i32(0)                   # timeDifference
    c.i32(0)                   # lastDcUpdateTime
    c.i64(struct.unpack("<q", os.urandom(8))[0])   # pushSessionId
    c.boolean(False)           # registeredForInternalPush
    c.i32(int(time.time()))
    c.i32(0)                   # sessionsToDestroy
    c.i32(1)                   # datacenters
    c.raw(bytes(d.b))
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "tgnet.dat"), "wb") as f:
        f.write(struct.pack("<I", len(c.b)) + bytes(c.b))

    u = s["user"]
    ub = Buf()
    ub.u32(0xB1B8CC83)                  # TL_user
    ub.i32(0x400 | 0x2 | 0x10)          # self | first_name | phone
    ub.i32(0)                           # flags2
    ub.i64(int(u["id"]))
    ub.string(u.get("first_name") or "User")
    ub.string(str(u.get("phone") or "").lstrip("+"))
    xml = (
        "<?xml version='1.0' encoding='utf-8' standalone='yes' ?>\n<map>\n"
        f'    <string name="user">{base64.b64encode(bytes(ub.b)).decode()}</string>\n'
        '    <boolean name="registeredForPush" value="false" />\n'
        "</map>\n"
    )
    open(os.path.join(out, "userconfing.xml"), "w").write(xml)
    print(f"tgnet.dat ({len(c.b)+4} bayt), userconfing.xml — dc={home}, kalit_id=...{key_id & 0xffff:04x}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
