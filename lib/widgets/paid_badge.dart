// lib/widgets/paid_badge.dart — "PULLIK" BELGISI.
//
// TALAB (foydalanuvchi): "pullik animelar kartochkasi ustiga pullik
// ekanini bildiradigan belgi" va "kartochkalar qayerda chiqsa barchasida
// chiqsin". Shu sabab belgi bitta joyda: bosh sahifa, katalog,
// sevimlilar, statistika, qidiruv, tarix, yuklanmalar va pleyerdagi
// bo'limlar ro'yxati shu vidjetni ishlatadi.
//
// Qaysi bo'lim pullik — SERVER hal qiladi (`free` maydoni,
// `seasons_repo.dart` -> `seasonIsPaid`). Ma'lumot yo'q bo'lsa belgi
// chiqmaydi: bepul bo'limga noto'g'ri "Pullik" yozilib qolmasin.

import 'package:flutter/material.dart';

import '../services/seasons_repo.dart';
import 'glass.dart';

/// Oltin toj + "Pullik".
class PaidBadge extends StatelessWidget {
  /// Kichik (ro'yxat qatorlari, kichik posterlar uchun) — faqat toj.
  final bool compact;
  const PaidBadge({super.key, this.compact = false});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.symmetric(
          horizontal: compact ? 4 : 7, vertical: compact ? 3 : 2.5),
      decoration: BoxDecoration(
        color: Colors.black.withValues(alpha: 0.62),
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: AppColors.gold.withValues(alpha: 0.7)),
      ),
      child: compact
          ? const Icon(Icons.workspace_premium_rounded,
              size: 13, color: AppColors.gold)
          : const Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.workspace_premium_rounded,
                    size: 12, color: AppColors.gold),
                SizedBox(width: 3),
                Text(
                  'Pullik',
                  style: TextStyle(
                    color: AppColors.gold,
                    fontSize: 10.5,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ],
            ),
    );
  }
}

/// Bo'lim pullik bo'lsa — belgi, aks holda hech narsa.
/// `season` — kamida `anime_id` va `season_id` (yoki `free`) bo'lgan obyekt.
class PaidMark extends StatelessWidget {
  final Map<String, dynamic> season;
  final bool compact;
  const PaidMark({super.key, required this.season, this.compact = false});

  /// Faqat `anime_id` va `season_id` bilan.
  PaidMark.ids(int animeId, int seasonId, {super.key, this.compact = false})
      : season = {'anime_id': animeId, 'season_id': seasonId};

  @override
  Widget build(BuildContext context) => seasonIsPaid(season)
      ? PaidBadge(compact: compact)
      : const SizedBox.shrink();
}
