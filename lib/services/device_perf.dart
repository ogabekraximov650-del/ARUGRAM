// lib/services/device_perf.dart — TELEFON QANCHALIK KUCHLI.
//
// TALAB (foydalanuvchi): "kuchsiz telefonlarda ham qotmasin — Telegram
// qanday qilsa shunday".
//
// Telegram (`SharedConfig.getDevicePerformanceClass`) telefonni uch
// sinfga ajratadi — yadrolar soni, protsessorning eng yuqori chastotasi
// va xotira bo'yicha — va kuchsizida animatsiyalarni o'zi kamaytiradi
// (`LiteMode`). Bu yerda ham xuddi shunday; qiymatlar `/sys` va
// `/proc/meminfo` dan to'g'ridan-to'g'ri o'qiladi (ruxsat kerak emas),
// bir marta.
//
//   * KUCHSIZ: ≤ 2 yadro; yoki ≤ 4 yadro va ≤ 1.6 GHz; yoki ≤ 2 GB xotira;
//   * KUCHLI: ≥ 8 yadro, ≥ 2.05 GHz va ≥ 6 GB xotira;
//   * qolgani — O'RTACHA.

import 'dart:io';

enum PerfClass { low, average, high }

class DevicePerf {
  DevicePerf._();

  static final PerfClass cls = _detect();

  static bool get low => cls == PerfClass.low;
  static bool get high => cls == PerfClass.high;

  static PerfClass _detect() {
    if (!Platform.isAndroid) return PerfClass.high;
    try {
      final cores = Platform.numberOfProcessors;
      final mhz = _maxFreqMhz(cores);
      final ramGb = _ramGb();
      if (cores <= 2 ||
          (cores <= 4 && mhz > 0 && mhz <= 1600) ||
          (ramGb > 0 && ramGb <= 2.2)) {
        return PerfClass.low;
      }
      if (cores >= 8 && (mhz <= 0 || mhz >= 2050) && ramGb >= 5.5) {
        return PerfClass.high;
      }
      return PerfClass.average;
    } catch (_) {
      return PerfClass.average;
    }
  }

  /// Eng tez yadroning chastotasi (MHz); o'qib bo'lmasa 0.
  static int _maxFreqMhz(int cores) {
    var best = 0;
    for (var i = 0; i < cores; i++) {
      try {
        final f = File(
            '/sys/devices/system/cpu/cpu$i/cpufreq/cpuinfo_max_freq');
        final khz = int.tryParse(f.readAsStringSync().trim()) ?? 0;
        if (khz ~/ 1000 > best) best = khz ~/ 1000;
      } catch (_) {}
    }
    return best;
  }

  /// Umumiy xotira (GB); o'qib bo'lmasa 0.
  static double _ramGb() {
    try {
      for (final l in File('/proc/meminfo').readAsLinesSync()) {
        if (l.startsWith('MemTotal:')) {
          final kb = int.tryParse(l.replaceAll(RegExp(r'[^0-9]'), '')) ?? 0;
          return kb / 1024 / 1024;
        }
      }
    } catch (_) {}
    return 0;
  }
}
