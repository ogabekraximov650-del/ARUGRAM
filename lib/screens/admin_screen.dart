import 'package:flutter/material.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../services/admin_badges.dart';
import 'admin_app_screen.dart';
import 'admin_channels_screen.dart';
import 'admin_encode_screen.dart';
import 'stats_screen.dart';
import 'admin_reports_screen.dart';
import 'admin_packs_screen.dart';
import 'admin_users_screen.dart';
import 'anime_management_screen.dart';

class AdminScreen extends StatelessWidget {
  const AdminScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        body: SafeArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // ── Header ───────────────────────────────────────────────
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
                child: Glass(
                  borderRadius: 20,
                  blur: 16,
                  padding:
                      const EdgeInsets.symmetric(horizontal: 18, vertical: 16),
                  child: Row(
                    children: [
                      GlassTappable(
                        onTap: () => Navigator.of(context).pop(),
                        child: const Glass(
                          borderRadius: 14,
                          blur: 14,
                          padding: EdgeInsets.all(8),
                          child: Icon(Icons.arrow_back_rounded,
                              color: Colors.white),
                        ),
                      ),
                      const SizedBox(width: 14),
                      const Expanded(
                        child: Text('Admin paneli',
                            style: TextStyle(
                                fontSize: 22,
                                fontWeight: FontWeight.bold,
                                color: Colors.white)),
                      ),
                      const Icon(Icons.admin_panel_settings_rounded,
                          color: Colors.white54),
                    ],
                  ),
                ),
              ),

              // ── Boshqaruv tugmalari ──────────────────────────────────
              Expanded(
                child: ListView(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
                  children: [
                    _AdminButton(
                      icon: Icons.movie_filter_rounded,
                      label: 'Animelarni boshqarish',
                      subtitle: "Qo'shish, tahrirlash, o'chirish",
                      onTap: () => Navigator.of(context).push(
                        PageRouteBuilder(
                          transitionDuration: const Duration(milliseconds: 320),
                          // const olib tashlandi
                          pageBuilder: (_, animation, __) =>
                              AnimeManagementScreen(),
                          transitionsBuilder: (_, animation, __, child) {
                            final curved = CurvedAnimation(
                                parent: animation, curve: Curves.easeOutCubic);
                            return FadeTransition(
                              opacity: curved,
                              child: SlideTransition(
                                position: Tween<Offset>(
                                        begin: const Offset(1, 0),
                                        end: Offset.zero)
                                    .animate(curved),
                                child: child,
                              ),
                            );
                          },
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    // ── FOYDALANUVCHILAR VA YOZISHMALAR ────────
                    //
                    // TALAB (foydalanuvchi): "foydalanuvchi
                    // boshqaruvini barcha suhbatlarga ulay
                    // olasanmi, ya'ni bitta bo'lim orqali ishlash
                    // qulayroq bo'lar edi".
                    //
                    // Ilgari ikkita alohida tugma edi. Endi bitta:
                    // yozishmalar o'sha ekranda uchinchi varaq
                    // bo'lib turadi.
                    //
                    // O'qilmagan xabar bo'lsa tugmada soni
                    // ko'rinadi — bo'limga kirmasdan turib
                    // bilinadi.
                    AnimatedBuilder(
                      animation: AdminBadges.instance,
                      builder: (context, _) {
                        final b = AdminBadges.instance;
                        // Tugmadagi son — o'qilmagan xabarlar.
                        // Yangi hisoblar esa izohda ko'rinadi:
                        // ikkovini bitta raqamga qo'shib yuborish
                        // "nechta xabar bor" degan savolga
                        // noto'g'ri javob berardi.
                        final n = b.chat;
                        return _AdminButton(
                          icon: Icons.people_alt_rounded,
                          label: 'Foydalanuvchilar',
                          subtitle: _usersSubtitle(b),
                          badge: n,
                          dot: b.users > 0,
                          onTap: () {
                            // Bo'lim ochildi — "yangi hisob"
                            // nuqtasi so'nadi. Xabar nuqtasi esa
                            // yozishma ochilgach o'zi so'nadi.
                            b.markSeen('users');
                            Navigator.of(context).push(
                              MaterialPageRoute<void>(
                                builder: (_) => const AdminUsersScreen(),
                              ),
                            );
                          },
                        );
                      },
                    ),
                    const SizedBox(height: 12),
                    // ── SHIKOYATLAR ───────────────────────────
                    //
                    // TALAB (foydalanuvchi): "admin panelida
                    // shikoyatlar bo'limi bo'lsin: shikoyat
                    // qayerdan kelgani, shikoyat qilingan izoh va
                    // shikoyat qiluvchining xabari tursin; tagida
                    // Tekshirish, Xabar yuborish va Tozalash
                    // tugmalari bo'lsin".
                    AnimatedBuilder(
                      animation: AdminBadges.instance,
                      builder: (context, _) {
                        final n = AdminBadges.instance.reports;
                        return _AdminButton(
                          icon: Icons.flag_rounded,
                          label: 'Shikoyatlar',
                          subtitle: n > 0
                              ? '$n ta yangi shikoyat'
                              : 'Izohlarga kelgan shikoyatlar',
                          badge: n,
                          onTap: () {
                            AdminBadges.instance.markSeen('reports');
                            Navigator.of(context).push(
                              MaterialPageRoute<void>(
                                builder: (_) => const AdminReportsScreen(),
                              ),
                            );
                          },
                        );
                      },
                    ),
                    const SizedBox(height: 12),
                    // ── EMOJI, GIF VA STIKERLAR ───────────────
                    //
                    // Foydalanuvchilar yuklagan har bir rasm avval shu
                    // yerda tekshiriladi (`admin_packs_screen.dart`).
                    _AdminButton(
                      icon: Icons.emoji_emotions_rounded,
                      label: 'Emoji, GIF va stikerlar',
                      subtitle: 'Yuklangan rasmlarni ko\'rib chiqish',
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (_) => const AdminPacksScreen(),
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    // Bepul bo'lim uchun majburiy kanallar
                    // (`admin_channels_screen.dart`).
                    _AdminButton(
                      icon: Icons.campaign_rounded,
                      label: 'Majburiy obunalar',
                      subtitle: 'Kanallar, limit va statistika',
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (_) => const AdminChannelsScreen(),
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    // ── UMUMIY STATISTIKA ─────────────────────
                    //
                    // Bosh sahifadagi banner build'da o'chiq
                    // (`kHomeStats`), shu sabab statistika sahifasiga
                    // admin panelidan kiriladi.
                    _AdminButton(
                      icon: Icons.insights_rounded,
                      label: 'Umumiy statistika',
                      subtitle: 'Kontent, foydalanuvchilar, ko\'rishlar, trafik',
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (_) => const StatsScreen(),
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    // ── KODLASH NAVBATI ───────────────────────
                    //
                    // Hozir kodlanayotgan va navbatdagi qismlar,
                    // jonli log (`admin_encode_screen.dart`).
                    _AdminButton(
                      icon: Icons.movie_filter_rounded,
                      label: 'Kodlash navbati',
                      subtitle: 'Kodlanayotgan va navbatdagi qismlar, log',
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (_) => const AdminEncodeScreen(),
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    // ── ILOVA VA XAVFSIZLIK ───────────────────
                    //
                    // TALAB (foydalanuvchi): "admin panelga
                    // versiya raqam yozadigan bo'lim qo'sh ...
                    // Worker shu va shundan katta versiyalarda
                    // ishlaydi" va "ulanish kaliti yasab bazaga
                    // qo'sh".
                    _AdminButton(
                      icon: Icons.security_rounded,
                      label: 'Ilova va xavfsizlik',
                      subtitle: 'Eng past versiya, ulanish kaliti',
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (_) => const AdminAppScreen(),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Foydalanuvchilar tugmasining izohi.
///
/// Ikki xil yangilik bo'lishi mumkin: o'qilmagan xabar va yangi
/// ro'yxatdan o'tganlar. Ikkovi ham bo'lsa ikkovi ham yoziladi —
/// admin qaysi biri uchun kirayotganini biladi.
String _usersSubtitle(AdminBadges b) {
  final parts = <String>[
    if (b.chat > 0) '${b.chat} ta o\'qilmagan xabar',
    if (b.users > 0) '${b.users} ta yangi hisob',
  ];
  if (parts.isEmpty) return 'Balans, obuna, bloklash, yozishma';
  return parts.join(' · ');
}

class _AdminButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final String subtitle;
  final VoidCallback onTap;

  /// O'qilmaganlar soni (0 — belgi ko'rsatilmaydi).
  final int badge;

  /// Soni yo'q, lekin yangilik bor (masalan yangi hisob).
  ///
  /// `badge` bilan bir vaqtda kelsa — son ko'rsatiladi, chunki
  /// son nuqtadan ko'proq narsa aytadi.
  final bool dot;

  const _AdminButton({
    required this.icon,
    required this.label,
    required this.subtitle,
    required this.onTap,
    this.badge = 0,
    this.dot = false,
  });

  @override
  Widget build(BuildContext context) {
    return GlassTappable(
      onTap: onTap,
      child: Glass(
        borderRadius: 20,
        blur: 16,
        tint: 0.12,
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 18),
        child: Row(
          children: [
            Container(
              padding: const EdgeInsets.all(12),
              // Rasmdagi uslub: apelsin tusli yumaloq kvadratcha,
              // ichida apelsin belgi (oq emas).
              decoration: BoxDecoration(
                color: AppColors.accent.withValues(alpha: 0.14),
                borderRadius: BorderRadius.circular(14),
              ),
              child: Icon(icon, color: AppColors.accent, size: 26),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(label,
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 16,
                          fontWeight: FontWeight.w600)),
                  const SizedBox(height: 3),
                  Text(subtitle,
                      style: TextStyle(
                          color: Colors.white.withValues(alpha: 0.55), fontSize: 12)),
                ],
              ),
            ),
            // Soni yo'q yangilik — oddiy nuqta.
            if (badge <= 0 && dot) ...[
              Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  color: AppColors.accent,
                  shape: BoxShape.circle,
                  boxShadow: [
                    BoxShadow(
                      color: AppColors.accent.withValues(alpha: 0.6),
                      blurRadius: 7,
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
            ],
            if (badge > 0) ...[
              Container(
                constraints: const BoxConstraints(minWidth: 22),
                padding:
                    const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
                decoration: BoxDecoration(
                  color: AppColors.accent,
                  borderRadius: BorderRadius.circular(11),
                ),
                child: Text(
                  '$badge',
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 11.5,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              const SizedBox(width: 6),
            ],
            const Icon(Icons.chevron_right_rounded, color: Colors.white38),
          ],
        ),
      ),
    );
  }
}
