# ytjob — GitHub Actions ishchisi (ytbot)

UZ: Ample'dagi `ytbot` navbat, log va chat xabarini boshqaradi; og'ir ishlar (katta yuklash, bo'lish, yuborish) ikkinchi GitHub akkauntdagi repoda bajariladi.
EN: `ytbot` on Ample manages the queue, logs and the chat message; heavy work runs in a repo of the second GitHub account.

## Ulash / Setup
1. UZ: Ikkinchi akkauntda YOPIQ repo oching; `ytjob.workflow.yml` ni `.github/workflows/ytjob.yml` qilib `main` branchga qo'ying.
   EN: Create a PRIVATE repo on the second account; add `ytjob.workflow.yml` as `.github/workflows/ytjob.yml` on `main`.
2. UZ: Shu akkauntda fine-grained token yarating: faqat shu repo, "Actions: Read and write" huquqi.
   EN: Create a fine-grained token on that account: only that repo, permission "Actions: Read and write".
3. UZ: Tokenni Ample yuklash sahifasi orqali `gh_token` nomli fayl qilib bering; ishga tushirish skripti uni `encode_token` kaliti bilan SHIFRLAB `/var/lib/ytbot/gh.enc` ga yozadi va ochiq nusxasini o'chiradi.
   EN: Hand the token over through the Ample upload page as a file named `gh_token`; the start script ENCRYPTS it with the `encode_token` key into `/var/lib/ytbot/gh.enc` and shreds the plain copy.
4. UZ: Deploy muhitiga `GH_REPO=egasi/repo` qo'shing. Botda `/actions` holatni ko'rsatadi.
   EN: Add `GH_REPO=owner/repo` to the deploy env. `/actions` in the bot shows the status.

## Qanday ishlaydi / How it works
- UZ: >400 MB yoki diskka sig'maydigan ish Actions'ga topshiriladi. Ample ishga xos bir martalik kalit yaratadi, workflow'ni ishga tushiradi va tugaguncha kutadi (navbat birin-ketin).
  EN: Jobs over 400 MB (or that do not fit the disk) go to Actions. Ample makes a one-time per-job key, triggers the workflow and waits (the queue stays one-by-one).
- UZ: Ishchi `ytbot ajob` dasturini Ample'dan (`/bin`) oladi, ish ma'lumoti va cookies'ni `/ajob`, `/ack` orqali oladi (GitHub'da saqlanmaydi), logni har soniyada `/prog` ga yuboradi.
  EN: The worker fetches `ytbot ajob` from Ample (`/bin`), gets job data and cookies through `/ajob`, `/ack` (never stored on GitHub) and posts its log to `/prog` every second.
- UZ: Ample chat xabarini kamida 3 soniyada bir yangilaydi (Telegram cheklovi).
  EN: Ample edits the chat message at most once per 3 seconds (Telegram limits).
- UZ: Ish tugagach kalit o'chiriladi. Repo YOPIQ bo'lishi kerak: workflow kirishlari (kalit) ishga tushirish sahifasida ko'rinadi.
  EN: The key is wiped when the job ends. The repo must be PRIVATE: workflow inputs (the key) are visible on the run page.
