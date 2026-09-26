# lib/widgets/tg_emoji_data.dart ni yasaydi (panel tartibi, fixEmoji, teri rangi to'plami).
import re, sys
from emoji_data import *
i=java.index('char[] emojiToFE0F = {'); j=java.index('};',i)
fe0f={int(x,16) for x in re.findall(r'0x([0-9A-Fa-f]+)', java[i:j])}
SPECIAL={0xDE2F,0xDC04,0xDE1A,0xDD7F,0xDFF3,0xDF2B,0xDC41,0xDD75,0xDFCC,0xDFCB}
def fix(e):
    # Emoji.fixEmoji — UTF-16 bo'yicha
    u=e.encode('utf-16-le'); units=[int.from_bytes(u[k:k+2],'little') for k in range(0,len(u),2)]
    out=[]; a=0
    while a<len(units):
        ch=units[a]
        if 0xD83C<=ch<=0xD83E:
            out.append(ch)
            if a+1<len(units):
                nx=units[a+1]; out.append(nx); a+=2
                if ch==0xD83C and nx in SPECIAL and (a>=len(units) or units[a]!=0xFE0F): out.append(0xFE0F)
                continue
            a+=1; continue
        if ch==0x20E3: out+=units[a:]; break
        out.append(ch)
        if 0x23<=ch<=0x3299 and ch in fe0f and (a+1>=len(units) or units[a+1]!=0xFE0F): out.append(0xFE0F)
        a+=1
    return b''.join(x.to_bytes(2,'little') for x in out).decode('utf-16-le')
def dq(s): return "'"+s.replace('\\\\','\\\\\\\\').replace("'","\\\\'")+"'"
titles=[('smileys',"Emoji va odamlar"),('animals',"Hayvonlar va tabiat"),('food',"Ovqat va ichimliklar"),('activity',"Faoliyat"),('travel',"Sayohat va joylar"),('objects',"Buyumlar"),('symbols',"Belgilar"),('flags',"Bayroqlar")]
o=["// lib/widgets/tg_emoji_data.dart — emoji paneli Telegram'dagidek.",
"// Manba: Cherrygram `EmojiData.dataColored` (bo'limlar va tartib),",
"// `Emoji.fixEmoji` (kerakli joyga U+FE0F). Rasmlar — `fonts/TgEmoji.ttf`.",
"// AVTOMATIK YASALGAN (`tool/tg_emoji/`) — qo'lda tahrirlamang.","",
"class TgEmojiGroup {","  final String id;","  final String title;","  final List<String> emoji;","  const TgEmojiGroup(this.id, this.title, this.emoji);","}","",
"const tgEmojiGroups = <TgEmojiGroup>["]
for (gid,t),sec in zip(titles,colored):
    o.append(f"  TgEmojiGroup('{gid}', \"{t}\", [{','.join(dq(fix(e)) for e in sec)}]),")
o.append("];")
colored_list=arr1('emojiColored')
o.append("")
o.append("/// Teri rangini tanlasa bo'ladigan emojilar (U+FE0F siz) —")
o.append("/// `EmojiData.emojiColored`; bosib turilsa rang tanlash oynasi.")
o.append("const tgEmojiColored = <String>{" + ",".join(dq(x.replace('\ufe0f','')) for x in colored_list) + "};")
open(sys.argv[1],'w',encoding='utf-8').write('\n'.join(o)+'\n')
print(fix('☺')=='☺️', fix('❤')=='❤️', fix('😀')=='😀')
