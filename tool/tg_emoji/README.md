# Telegram emoji shrifti (`fonts/TgEmoji.ttf`)

Oddiy emoji Telegram serverida YO'Q — rasmlar Telegram ilovasining
o'zida. Bu yerda ular Cherrygram (`arsLan4k1390/Cherrygram`) dagi
`TMessagesProj/src/main/assets/emoji` dan olinib, rangli CBDT shriftga
yig'iladi (Noto Color Emoji yig'uvchisi bilan).

```sh
pip install pillow imagequant fonttools notofonttools
git clone --depth 1 --filter=blob:none --sparse https://github.com/arsLan4k1390/Cherrygram.git cg
(cd cg && git sparse-checkout set --no-cone 'TMessagesProj/src/main/assets/emoji/*' \
   '/TMessagesProj/src/main/java/org/telegram/messenger/EmojiData.java')
git clone --depth 1 https://github.com/googlefonts/noto-emoji.git ne
cp tool/tg_emoji/*.py .                      # SRC = cg/... (emoji_data.py)
python3 make_pngs.py                         # -> pngs/
python3 gen_dart.py lib/widgets/tg_emoji_data.dart
python3 ne/add_glyphs.py -f ne/NotoColorEmoji.tmpl.ttx.tmpl -o t.ttx -d pngs
ttx -q -o t.ttf t.ttx
python3 ne/third_party/color_emoji/emoji_builder.py -S -V t.ttf out.ttf pngs/emoji_u
add_vs_cmap.py -vs 2640 2642 2695 --dstdir . -o fonts/TgEmoji.ttf out.ttf
```
So'ng `name` jadvalidagi oila nomi `TgEmoji` qilib qo'yilgan (fontTools).
