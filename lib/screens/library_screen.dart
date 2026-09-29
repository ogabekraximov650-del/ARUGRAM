// lib/screens/library_screen.dart — KUTUBXONA.
//
// Uchta oyna (foydalanuvchi talabi, 2026-09-29: "Tarix va Sevimlilar
// o'rtasiga Yuklanmalar tugmasini qo'sh"):
//
//   1. Tarix       — faqat anime bo'yicha (`HistoryTab`);
//   2. Yuklanmalar — `DownloadsList`;
//   3. Sevimlilar  — `FavoritesTab`.
//
// Oynalar `PageView` da: tugma bosilsa ham, barmoq bilan o'ngga-chapga
// surilsa ham almashadi; ochilgan oyna holati (masalan tarix
// ro'yxatining o'rni) saqlanib qoladi.

import 'package:flutter/material.dart';

import '../widgets/glass.dart';
import 'favorites_screen.dart';
import 'history_screen.dart';

class LibraryScreen extends StatefulWidget {
  const LibraryScreen({super.key});

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  int _tab = 0;

  // ── BARMOQ BILAN SURIB O'TISH (foydalanuvchi talabi, 2026-09-29) ──
  //
  // "Ekranni qo'lda o'ngga yoki chapga surib Tarix, Yuklanmalar va
  // Sevimlilar oynalariga o'tsa bo'ladigan qil". `IndexedStack` o'rniga
  // `PageView`: tugma ham, surish ham bitta sahifani suzdiradi. Har
  // oyna tirik saqlanadi (`_KeepAlive`) — ro'yxat o'rni yo'qolmaydi.
  final PageController _pages = PageController();

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _goTo(int i) {
    if (i == _tab) return;
    _pages.animateToPage(
      i,
      duration: const Duration(milliseconds: 260),
      curve: Curves.easeOutCubic,
    );
  }

  static const _titles = ['Tarix', 'Yuklanmalar', 'Sevimlilar'];

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        const Padding(
          padding: EdgeInsets.fromLTRB(20, 16, 20, 12),
          child: Row(
            children: [
              Text('Kutubxona',
                  style: TextStyle(
                      fontSize: 22,
                      fontWeight: FontWeight.bold,
                      color: Colors.white)),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: Row(
            children: List.generate(_titles.length, (i) {
              return Expanded(
                child: Padding(
                  padding: EdgeInsets.only(right: i == _titles.length - 1 ? 0 : 8),
                  child: _TabButton(
                    label: _titles[i],
                    active: _tab == i,
                    onTap: () => _goTo(i),
                  ),
                ),
              );
            }),
          ),
        ),
        Expanded(
          child: PageView(
            controller: _pages,
            physics: const BouncingScrollPhysics(),
            // Qo'shni oyna oldindan quriladi — surishda qotish yo'q.
            allowImplicitScrolling: true,
            onPageChanged: (i) => setState(() => _tab = i),
            children: const [
              _KeepAlive(child: HistoryTab()),
              _KeepAlive(child: DownloadsList()),
              _KeepAlive(child: FavoritesTab()),
            ],
          ),
        ),
      ],
    );
  }
}

/// Sahifani `PageView` da tirik saqlaydi (ro'yxat o'rni yo'qolmasin).
class _KeepAlive extends StatefulWidget {
  final Widget child;
  const _KeepAlive({required this.child});

  @override
  State<_KeepAlive> createState() => _KeepAliveState();
}

class _KeepAliveState extends State<_KeepAlive>
    with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return widget.child;
  }
}

/// Tugma — ilgari Tarix ichidagi "Anime bo'yicha / Qism bo'yicha"
/// tugmalari bilan AYNAN bir xil ko'rinish (foydalanuvchi talabi):
/// faol — apelsin gradient, qolganlari — karta rangida.
class _TabButton extends StatelessWidget {
  final String label;
  final bool active;
  final VoidCallback onTap;
  const _TabButton(
      {required this.label, required this.active, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 180),
        padding: const EdgeInsets.symmetric(vertical: 11),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(14),
          gradient: active
              ? const LinearGradient(
                  colors: [AppColors.accent, AppColors.accent2],
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                )
              : null,
          color: active ? null : AppColors.card,
          border: Border.all(
              color: active ? Colors.transparent : AppColors.border, width: 1),
        ),
        child: Center(
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: active ? Colors.white : Colors.white60,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ),
    );
  }
}
