// lib/screens/stats_screen.dart — UMUMIY STATISTIKA.
//
// Bitta sahifa, oltita blok (ekranni yuqoriga surib ko'riladi):
//
//   Foydalanuvchilar → Anime ko'rishlar → Bo'lim ko'rishlar →
//   Qism ko'rishlar → Ko'rish vaqti → Trafik sarfi
//
// Har blok ostida treyding chizig'idek grafik (`trend_chart.dart`).
//
// Har birida bir xil tartib: Kunlik / Haftalik / Oylik / Umumiy.
// Raqamlar uch xonadan ajratiladi (`1.000`). Yillik ko'rsatkich
// ATAYLAB yo'q — foydalanuvchi talabi.
//
// Kunlik ko'rsatkich — OXIRGI 24 SOAT (foydalanuvchi talabi),
// qolganlari esa Toshkent (UTC+5) kunlari bo'yicha.

import 'package:flutter/material.dart';

import '../services/format.dart';
import '../services/stats_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/trend_chart.dart';

class StatsScreen extends StatefulWidget {
  const StatsScreen({super.key});

  @override
  State<StatsScreen> createState() => _StatsScreenState();
}

class _StatsScreenState extends State<StatsScreen> {
  @override
  void initState() {
    super.initState();
    // Sahifa ochilganda eng yangi raqamlar so'raladi (10 daqiqa
    // ichida qayta so'ralmaydi).
    StatsService.instance.load();
  }

  @override
  Widget build(BuildContext context) {
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        body: SafeArea(
          child: AnimatedBuilder(
            animation: StatsService.instance,
            builder: (context, _) {
              final s = StatsService.instance;
              return Column(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16, 12, 16, 10),
                    child: Row(
                      children: [
                        GestureDetector(
                          onTap: () => Navigator.of(context).pop(),
                          behavior: HitTestBehavior.opaque,
                          child: const Glass(
                            borderRadius: 14,
                            padding: EdgeInsets.all(8),
                            child: Icon(Icons.arrow_back_rounded,
                                color: Colors.white),
                          ),
                        ),
                        const SizedBox(width: 14),
                        const Expanded(
                          child: Text(
                            'Umumiy statistika',
                            style: TextStyle(
                              color: Colors.white,
                              fontSize: 18,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        if (s.isLoading)
                          const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: Colors.white54),
                          ),
                      ],
                    ),
                  ),
                  Expanded(
                    child: RefreshIndicator(
                      color: AppColors.accent,
                      backgroundColor: AppColors.card,
                      onRefresh: () => s.load(force: true),
                      child: ListView(
                        physics: const BouncingScrollPhysics(
                            parent: AlwaysScrollableScrollPhysics()),
                        padding: const EdgeInsets.fromLTRB(16, 4, 16, 120),
                        children: [
                          // TALAB (foydalanuvchi): kunlik, haftalik, oylik
                          // va umumiy — foydalanuvchilar, anime / bo'lim /
                          // qism ko'rishlar, ko'rish soati, trafik; har
                          // biri treyding chizig'idek grafik bilan.
                          _StatCard(
                            title: 'Foydalanuvchilar',
                            icon: Icons.people_alt_rounded,
                            block: s.stats.users,
                            format: (v) => '${formatCount(v)} ta',
                            note: 'Kunlik — oxirgi 24 soatda kirganlar. '
                                'Grafik — yangi ochilgan hisoblar',
                            metric: 'users',
                          ),
                          const SizedBox(height: 12),
                          _StatCard(
                            title: 'Anime ko\'rishlar',
                            icon: Icons.movie_filter_rounded,
                            block: s.stats.animeViews,
                            format: (v) => '${formatCount(v)} ta',
                            note: 'Bitta odam bitta animeni ko\'rgani bir marta '
                                'sanaladi',
                            metric: 'anime_views',
                          ),
                          const SizedBox(height: 12),
                          _StatCard(
                            title: 'Bo\'lim ko\'rishlar',
                            icon: Icons.video_library_rounded,
                            block: s.stats.seasonViews,
                            format: (v) => '${formatCount(v)} ta',
                            note: 'Bitta odam bitta bo\'limni ko\'rgani bir marta '
                                'sanaladi',
                            metric: 'season_views',
                          ),
                          const SizedBox(height: 12),
                          _StatCard(
                            title: 'Qism ko\'rishlar',
                            icon: Icons.play_circle_fill_rounded,
                            block: s.stats.views,
                            format: (v) => '${formatCount(v)} ta',
                            note: 'Qism ochilib ko\'rilgani hisoblanadi',
                            metric: 'views',
                          ),
                          const SizedBox(height: 12),
                          _StatCard(
                            title: 'Ko\'rish vaqti',
                            icon: Icons.schedule_rounded,
                            block: s.stats.watch,
                            format: (v) => '${formatHours(v)} soat',
                            note: '1x tezlikdagi haqiqiy vaqt',
                            metric: 'watch_ms',
                          ),
                          const SizedBox(height: 12),
                          _StatCard(
                            title: 'Trafik sarfi',
                            icon: Icons.cloud_download_rounded,
                            block: s.stats.traffic,
                            format: formatBytes,
                            note: 'Barcha foydalanuvchilar qabul qilgan hajm',
                            metric: 'traffic',
                          ),
                          const SizedBox(height: 16),
                          Center(
                            child: Text(
                              'Vaqt mintaqasi: UTC+5 (Toshkent)',
                              style: TextStyle(
                                color: Colors.white.withValues(alpha: 0.35),
                                fontSize: 11.5,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _StatCard extends StatelessWidget {
  final String title;
  final IconData icon;
  final StatBlock block;
  final String Function(int) format;
  final String note;

  /// Grafik ko'rsatkichi (`/api/stats/series`).
  final String metric;

  const _StatCard({
    required this.title,
    required this.icon,
    required this.block,
    required this.format,
    required this.note,
    required this.metric,
  });

  @override
  Widget build(BuildContext context) {
    return Glass(
      borderRadius: 20,
      padding: const EdgeInsets.fromLTRB(18, 16, 18, 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 20, color: AppColors.accent),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  title,
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 16.5,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          _row('Kunlik', block.daily),
          _row('Haftalik', block.weekly),
          _row('Oylik', block.monthly),
          _row('Umumiy', block.total, strong: true),
          const SizedBox(height: 14),
          TrendChart(metric: metric, format: format),
          const SizedBox(height: 8),
          Text(
            note,
            style: TextStyle(
              color: Colors.white.withValues(alpha: 0.35),
              fontSize: 11.5,
              height: 1.3,
            ),
          ),
        ],
      ),
    );
  }

  Widget _row(String label, int value, {bool strong = false}) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Text(
            '$label:',
            style: TextStyle(
              color: Colors.white.withValues(alpha: strong ? 0.85 : 0.6),
              fontSize: 13.5,
              fontWeight: strong ? FontWeight.w600 : FontWeight.w500,
            ),
          ),
          const Spacer(),
          Text(
            format(value),
            style: TextStyle(
              color: Colors.white,
              fontSize: strong ? 16 : 14.5,
              fontWeight: strong ? FontWeight.w800 : FontWeight.w600,
            ),
          ),
        ],
      ),
    );
  }
}
