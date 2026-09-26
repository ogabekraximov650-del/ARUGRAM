// lib/screens/support_chat_screen.dart — ADMIN BILAN YOZISHMA.
//
// ═══════════════════════════════════════════════════════════════
//  IKKI TOMON, BITTA EKRAN
// ═══════════════════════════════════════════════════════════════
//
// Bu ekran ikki joyda ishlatiladi:
//
//   * FOYDALANUVCHI — profil sahifasidagi "Admin bilan bog'lanish"
//     tugmasidan. `userId` berilmaydi, ya'ni o'z suhbati ochiladi;
//   * ADMIN — barcha suhbatlar ro'yxatidan bittasini bosganda.
//     `userId` beriladi va o'sha odamning suhbati ochiladi.
//
// Farqi faqat kimning xabari qaysi tomonda turishida: o'zining
// xabari O'NGDA, suhbatdoshiniki CHAPDA — Telegram'dagidek.

import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:cached_network_image/cached_network_image.dart';
import 'package:camera/camera.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';
import 'package:path_provider/path_provider.dart';
import 'package:record/record.dart';
import '../services/chat_video_thumb.dart';
import '../services/image_cache.dart';

import '../services/auth_service.dart';
import '../services/screen_guard.dart';
import '../services/support_service.dart';
import '../services/voice_player.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import 'media_view_screen.dart';
import 'public_profile_screen.dart';
import '../services/telegram_service.dart';
import '../services/tg_media.dart';
import '../widgets/tg_composer.dart';
import '../widgets/tg_media_view.dart';
import '../widgets/tg_record_button.dart';
import '../widgets/tg_attach_sheet.dart';
import '../widgets/tg_bubble.dart';
import '../widgets/tg_round_recorder.dart';
import '../widgets/tg_reply.dart';
import '../widgets/tg_waveform.dart';
import 'package:open_filex/open_filex.dart';
import '../widgets/emoji_text.dart';
import 'package:video_player/video_player.dart';
import 'package:flutter/foundation.dart' show defaultTargetPlatform, TargetPlatform;
import '../services/rust_bridge.dart';

class SupportChatScreen extends StatefulWidget {
  /// Admin boshqa odamning suhbatini ochsa — o'sha odamning raqami.
  final int? userId;

  /// Sarlavhada ko'rinadigan nom.
  final String title;

  /// Sarlavhadagi kichik rasm (admin ko'rinishida).
  final String photoUrl;

  const SupportChatScreen({
    super.key,
    this.userId,
    this.title = 'Admin bilan bog\'lanish',
    this.photoUrl = '',
  });

  @override
  State<SupportChatScreen> createState() => _SupportChatScreenState();
}

class _SupportChatScreenState extends State<SupportChatScreen>
    // ── SKRINSHOT VA EKRAN YOZUVI TAQIQLANADI ────────────────
    //
    // TALAB (foydalanuvchi): yozishmada ham skrinshot olish va
    // ekranni yozib olish taqiqlansin (`screen_guard.dart`).
    with ScreenGuarded<SupportChatScreen> {
  late final ChatController _chat = ChatController(userId: widget.userId);
  final _input = TgTextController();
  final _focus = FocusNode();
  final _scroll = ScrollController();
  bool _sending = false;

  // ── FAYL YUKLASH HOLATI ──────────────────────────────────
  //
  // TALAB (foydalanuvchi): "video yoki rasm yuborilayotganda
  // huddi admin video yuklagandagidek progress chiziqi va foiz
  // ko'rsatilsin, lekin progress chizig'i AYLANA ko'rinishda
  // bo'lsin".
  //
  // Shu sabab bu yerda ikkita son: 0..1 oralig'idagi ulush
  // (aylana shuni chizadi) va ko'rsatiladigan foiz.
  bool _uploading = false;
  double _upProgress = 0;

  // ── YUKLANAYOTGAN FAYL CHAT OYNASIDA ──────────────────────
  //
  // TALAB (foydalanuvchi): "video, ovozli xabar yoki rasm
  // yuborganda to'g'ridan-to'g'ri chat oynasida ko'rinsin va
  // progress chizig'i play/pause tugmasi atrofida aylanib
  // kattalashsin, huddi Telegramdagidek".
  //
  // Shu sabab yuklash davomida ro'yxatning oxiriga VAQTINCHALIK
  // puffak qo'yiladi: rasm bo'lsa o'zi ko'rinadi, video va ovoz
  // uchun esa tugma atrofida aylana to'lib boradi.
  String _upType = '';
  String _upPath = '';
  int _upMs = 0;

  /// Yozishmadagi videolarning kadrlari.
  ///
  /// Ekranga TEGISHLI (yagona/singleton EMAS): `dispose()` bilan
  /// birga butun ish to'xtaydi.
  final ChatVideoThumb _thumbs = ChatVideoThumb();

  // ── PASTDAN TORTIB YANGILASH ──────────────────────────────
  //
  // TALAB (foydalanuvchi): "chatdagi xabarlarni yuqoriga
  // ko'tarsa pastda aylanadigan narsa chiqsin va serverdan
  // chatga xabar kelgan-kelmaganini tekshirsin".
  //
  // Ro'yxatning oxiridan tashqariga chiqilgan masofa yig'iladi;
  // yetarli bo'lsa serverga so'rov ketadi.
  double _pullUp = 0;
  bool _pullBusy = false;

  // ── TANLASH REJIMI ────────────────────────────────────────
  //
  // TALAB (foydalanuvchi): "xabarni bittalab emas — ustiga bosib
  // turadi, xabar tanlandi, keyin qolganlarini qo'lda tanlab
  // o'chirsa bo'ladigan qil; va hammasini bittada tanlab
  // o'chiradigan tugma qo'sh. Chiqindi tugmasi o'ng yuqori
  // qismida bo'lsin. Va faqatgina admin o'chirishi mumkin
  // bo'lsin, foydalanuvchi o'chira olmasin".
  //
  // Bitta xabar uzoq bosilishi bilan rejim ochiladi; shundan
  // keyin oddiy bosish tanlaydi/tanlovni oladi. Ro'yxat bo'shashi
  // bilan rejim o'zi yopiladi.
  final Set<String> _selected = {};
  bool get _selecting => _selected.isNotEmpty;

  // ── OVOZLI XABAR ──────────────────────────────────────────
  //
  // TALAB (foydalanuvchi): "chatda ovozli xabar yuborish
  // tizimini ham qo'sh".
  //
  // Mikrofon tugmasi BOSILGANDA yozib olish boshlanadi va
  // tugmagacha davom etadi (barmoqni ushlab turish SHART EMAS).
  // Sabab: ushlab turish paytida ro'yxatni surish, ekranni
  // qulflash yoki tasodifiy qo'yib yuborish yozuvni yo'qotadi —
  // qo'yib yuborish bilan yozuv tugaydigan tizimda bu eng
  // ko'p uchraydigan shikoyat.
  final _rec = AudioRecorder();
  bool _recording = false;

  // ── TELEGRAM'DAGIDEK YOZISH (`tg_record_button.dart`) ─────
  /// Tepaga surib qulflangan — barmoq qo'yib yuborilsa ham yoziladi.
  bool _locked = false;

  /// Chapga surilgan masofa ("bekor qilish uchun suring").
  double _dragX = 0;

  /// Dumaloq video: old kamera.
  CameraController? _cam;
  bool _roundRec = false;

  /// Oxirgi dumaloq video yuborildimi (yopilish animatsiyasi uchun).
  bool _roundSent = false;
  Duration _recLen = Duration.zero;

  /// Yozish boshlangan payt — taymer soniyaning yuzdan biri bilan
  /// ko'rsatiladi (`TimerView`: "0:03,45").
  DateTime _recStart = DateTime.now();
  Timer? _recTimer;
  String? _recPath;

  /// Ovoz balandligi 0..1 — yozish doirasi shunga qarab to'lqinlanadi
  /// (Telegram: `amplitude / 1800`, 16 bitli namunalar RMS'i).
  final ValueNotifier<double> _amp = ValueNotifier(0);
  StreamSubscription<Amplitude>? _ampSub;

  /// Yozish davomidagi ovoz balandliklari — ovozli xabarning to'lqin
  /// shakli shulardan yasaladi (`tg_waveform.dart`).
  final List<double> _levels = [];

  // ── TELEGRAM'DAGIDEK: JAVOB, MENYU, PASTGA TUSHISH ──────────
  /// Javob berilayotgan xabar (yozish paneli tepasida qator).
  ChatMessage? _replyTo;

  /// Xabarlarning kalitlari — iqtibos bosilsa asl xabarga o'tish uchun.
  final Map<String, GlobalKey> _keys = {};

  /// Bir zum yoritiladigan xabar (iqtibosdan o'tilganda).
  String? _flash;

  /// Pastga tushish tugmasi ko'rinadimi (ro'yxat yuqoriga surilgan).
  bool _showDown = false;

  @override
  void initState() {
    super.initState();
    _chat.addListener(_onData);
    _scroll.addListener(() {
      if (!_scroll.hasClients) return;
      final p = _scroll.position;
      final down = p.pixels < p.maxScrollExtent - 400;
      if (down != _showDown) setState(() => _showDown = down);
    });
    // Avval DISK (darhol), keyin tarmoq.
    _chat.loadFromDisk();
    _chat.load().then((_) => _toBottom(jump: true));
    // Suhbat OCHIQ turgandagina yangi xabarlar so'raladi.
    _chat.startPolling();
  }

  @override
  void dispose() {
    // ── KADR YASASH SHU YERDA TO'XTAYDI ──────────────────
    //
    // Avvalgi urinish AYNAN shuning yo'qligidan yiqilgan edi:
    // boshlangan so'rovlar ekran yopilgandan keyin ham davom
    // etib, video ijrosiga qoladigan tezlikni yeb turardi
    // (`chat_video_thumb.dart` dagi tarixga qarang).
    _thumbs.dispose();
    // Ekran yopilsa ovoz ham to'xtaydi — aks holda u orqa fonda
    // yangrab qolardi.
    unawaited(VoicePlayer.instance.stop());
    _recTimer?.cancel();
    _ampSub?.cancel();
    _amp.dispose();
    unawaited(_rec.dispose());
    _chat.removeListener(_onData);
    _chat.stopPolling();
    _chat.dispose();
    _input.dispose();
    _focus.dispose();
    unawaited(_cam?.dispose());
    _scroll.dispose();
    // Ekran yopildi — profil sahifasidagi nuqta yangilansin.
    UnreadBadge.instance.refresh();
    super.dispose();
  }

  int _seen = 0;
  void _onData() {
    // Yangi xabar kelgan bo'lsa pastga tushamiz. Foydalanuvchi
    // yuqoriga surib eski xabarlarni o'qiyotgan bo'lsa —
    // TEGILMAYDI, aks holda ekran o'zidan o'zi sakrab ketardi.
    final n = _chat.items.length;
    if (n > _seen) {
      _seen = n;
      _toBottom();
    }
  }

  /// Ro'yxat pastdan tortildimi — shunda yangilanadi.
  bool _onScroll(ScrollNotification n) {
    if (_pullBusy) return false;
    if (n is OverscrollNotification) {
      // Musbat `overscroll` — OXIRIDAN tashqariga chiqish.
      if (n.overscroll > 0) {
        _pullUp += n.overscroll;
        if (_pullUp > 90) {
          _pullUp = 0;
          unawaited(_pullRefresh());
        }
      }
    } else if (n is ScrollEndNotification) {
      _pullUp = 0;
    }
    return false;
  }

  Future<void> _pullRefresh() async {
    if (_pullBusy) return;
    setState(() => _pullBusy = true);
    await _chat.load(force: true);
    await UnreadBadge.instance.refresh();
    if (!mounted) return;
    setState(() => _pullBusy = false);
  }

  void _toBottom({bool jump = false}) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_scroll.hasClients) return;
      final max = _scroll.position.maxScrollExtent;
      // Pastga yaqin bo'lsagina o'zi tushadi.
      if (!jump && _scroll.position.pixels < max - 300) return;
      if (jump) {
        _scroll.jumpTo(max);
      } else {
        _scroll.animateTo(max,
            duration: const Duration(milliseconds: 220),
            curve: Curves.easeOut);
      }
    });
  }

  /// O'chirish MUMKINMI.
  ///
  /// TALAB (foydalanuvchi): "foydalanuvchi support chatda
  /// yuborgan narsalarini o'chira olmasin, o'chirish faqat admin
  /// panelida mumkin bo'lsin".
  ///
  /// Shu sabab ikkita shart: admin bo'lishi VA suhbat admin
  /// panelidan ochilgan bo'lishi (`userId` berilgan). Admin o'z
  /// profilidan "Admin bilan bog'lanish"ni ochsa — u yerda ham
  /// o'chirish yo'q.
  ///
  /// Server ham shunday tekshiradi (admin bo'lmasa 403), ya'ni
  /// o'zgartirilgan ilova bilan ham o'chirib bo'lmaydi.
  bool get _canDelete =>
      AuthService.instance.user?.isAdmin == true && widget.userId != null;

  /// Faylni B2'ga yuklaydi va xabar qilib yuboradi.
  ///
  /// Rasm, video va ovozli xabar — uchovi ham SHU yo'ldan
  /// o'tadi, farqi faqat turida va uzunligida.
  Future<void> _uploadAndSend({
    required File file,
    required String ext,
    required String type,
    required String contentType,
    int ms = 0,
    Future<void> Function()? cleanup,
    // Xabar matni: berilmasa — yozish maydonidagisi (ovoz va dumaloq
    // videoda — matnsiz).
    String? body,
  }) async {
    if (_uploading) return;
    Future<void> dropCopy() async {
      if (cleanup != null) await cleanup();
    }

    final ct = contentType;
    // Nom YUBORUVCHINING hisob raqami bilan boshlanadi: worker
    // boshqalarga faqat o'z nomlarini yozishga ruxsat beradi
    // (`tg_user_media`), ikki odamning fayli esa hech qachon bir xil
    // nom olmaydi.
    final me = AuthService.instance.user?.id ?? 0;
    final name = 'chat_${me}_${DateTime.now().millisecondsSinceEpoch}.$ext';

    setState(() {
      _uploading = true;
      _upProgress = 0;
      _upType = type;
      _upPath = file.path;
      _upMs = ms;
    });
    _toBottom();

    // ── KADR BU YERDA YASALMAYDI ────────────────────────────
    //
    // TALAB (foydalanuvchi): "serverga thumbnail yuklanmasin".
    //
    // Ilgari kadr shu yerda mahalliy fayldan ajratilib, video
    // bilan birga B2'ga yuklanardi va xabarda uning nomi
    // ketardi. Endi bunday emas: kadrni HAR BIR KO'RUVCHI o'zida
    // yasaydi va o'zida saqlaydi
    // (`lib/services/chat_video_thumb.dart`).
    //
    // Ya'ni ombor ham, xabar maydoni ham, yuklash qadami ham
    // kerak emas — yuborish endi soddaroq va tezroq.

    try {
      // ── TELEGRAM'GA (ilovaning o'zi orqali) ─────────────────
      // Fayl worker'dan o'tmaydi va hujjat emas, ODDIY ko'rinishda
      // (surat/video) yuboriladi. Hajm chegarasi — Telegram'niki.
      final upErr = await TelegramService.instance.uploadFile(
        file.path,
        name,
        ct,
        onProgress: (sent, total) {
          if (!mounted) return;
          setState(() => _upProgress = total > 0 ? sent / total : 0);
        },
      );
      if (upErr != null) throw upErr;
      final b2Name = name;

      // Fayl joyida — endi xabarning o'zi yuboriladi.
      final err = await _chat.send(
        // Ovozli xabarga matn qo'shilmaydi: yozayotgan matn
        // o'z holicha qolsin, keyin alohida yuboriladi.
        body ??
            (type == 'voice' || type == 'round' ? '' : _input.encoded.trim()),
        mediaFile: b2Name,
        mediaType: type,
        mediaMs: ms,
      );
      if (!mounted) return;
      if (err != null) {
        _snack(err);
      } else {
        if (body == null && type != 'voice' && type != 'round') {
          _input.clear();
        }
        _toBottom();
      }
    } catch (e) {
      if (mounted) _snack('Yuborilmadi: $e');
    } finally {
      await dropCopy();
      if (mounted) {
        setState(() {
          _uploading = false;
          _upProgress = 0;
          _upType = '';
          _upPath = '';
          _upMs = 0;
        });
      }
    }
  }

  // ══════════════════════════════════════════════════════════
  //  OVOZ YOZIB OLISH
  // ══════════════════════════════════════════════════════════

  /// Mikrofon bosildi — yozib olish boshlanadi.
  Future<void> _startRecording() async {
    if (_recording || _uploading) return;
    // Ruxsatni paketning o'zi so'raydi. Berilmasa — sababi
    // aytiladi, jim qolinmaydi.
    bool allowed = false;
    try {
      allowed = await _rec.hasPermission();
    } catch (_) {}
    if (!allowed) {
      if (mounted) _snack('Mikrofonga ruxsat berilmadi');
      return;
    }
    // Ovoz yozilayotganda ijro to'xtaydi — mikrofon va
    // karnayning bir vaqtda ishlashi keraksiz.
    await VoicePlayer.instance.stop();

    try {
      final dir = await getTemporaryDirectory();
      final path =
          '${dir.path}/voice_${DateTime.now().millisecondsSinceEpoch}.m4a';
      await _rec.start(
        // AAC/m4a — Android ham, iOS ham tug'ma qo'llaydi va
        // ExoPlayer uni bemalol o'ynatadi.
        const RecordConfig(
          encoder: AudioEncoder.aacLc,
          bitRate: 64000,
          sampleRate: 44100,
          numChannels: 1,
        ),
        path: path,
      );
      _recPath = path;
      _recLen = Duration.zero;
      _recStart = DateTime.now();
      _levels.clear();
      // dBFS -> 16 bitli RMS (`* 32767`) -> Telegram shkalasi (1800).
      _ampSub?.cancel();
      _ampSub = _rec
          .onAmplitudeChanged(const Duration(milliseconds: 100))
          .listen((a) {
        final lin = math.pow(10, a.current / 20).toDouble();
        _amp.value = (lin * 32767 / 1800).clamp(0.0, 1.0);
        _levels.add(lin);
      });
      setState(() => _recording = true);
      _recTimer?.cancel();
      _recTimer = Timer.periodic(const Duration(milliseconds: 200), (_) {
        if (!mounted) return;
        setState(() => _recLen += const Duration(milliseconds: 200));
        // Juda uzun yozuvni o'zi to'xtatadi: 5 daqiqadan uzun
        // ovozli xabar yozishmaga to'g'ri kelmaydi.
        if (_recLen.inMinutes >= 5) _stopRecording(send: true);
      });
    } catch (e) {
      if (mounted) _snack('Yozib bo\'lmadi: $e');
    }
  }

  /// Yozishni tugatadi. `send` bo'lsa yuboradi, aks holda
  /// faylni o'chirib tashlaydi.
  Future<void> _stopRecording({required bool send}) async {
    if (!_recording) return;
    _locked = false;
    _dragX = 0;
    _recTimer?.cancel();
    _recTimer = null;
    final len = _recLen;
    _ampSub?.cancel();
    _ampSub = null;
    _amp.value = 0;
    setState(() => _recording = false);

    String? path;
    try {
      path = await _rec.stop();
    } catch (_) {}
    path ??= _recPath;
    _recPath = null;
    if (path == null) return;

    final file = File(path);
    Future<void> drop() async {
      try {
        if (await file.exists()) await file.delete();
      } catch (_) {}
    }

    if (!send) {
      await drop();
      return;
    }
    // Tasodifan bosilgan tugma xabar bo'lib ketmasin.
    if (len.inMilliseconds < 700) {
      await drop();
      if (mounted) _snack('Juda qisqa');
      return;
    }
    await _uploadAndSend(
      file: file,
      ext: 'm4a',
      type: 'voice',
      contentType: 'audio/mp4',
      ms: len.inMilliseconds,
      cleanup: drop,
      // Telegram'dagidek to'lqin shakli (matn o'rnida, ko'rinmaydi).
      body: tgEncodeWaveform(_levels),
    );
  }

  void _snack(String text) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        behavior: SnackBarBehavior.floating,
        backgroundColor: AppColors.card,
        content: Text(text, style: const TextStyle(color: Colors.white)),
      ),
    );
  }

  /// Xabarni tanlaydi yoki tanlovdan chiqaradi.
  ///
  /// FAQAT ADMIN: foydalanuvchida tanlash umuman ochilmaydi.
  /// (Server ham shunday: o'chirish so'rovi admin bo'lmasa 403
  /// qaytaradi — ya'ni o'zgartirilgan ilova ham o'chira olmaydi.)
  void _toggleSelect(ChatMessage m) {
    if (!_canDelete) return;
    setState(() {
      if (!_selected.remove(m.id)) _selected.add(m.id);
    });
  }

  void _clearSelection() => setState(_selected.clear);

  void _selectAll() => setState(() {
        _selected
          ..clear()
          ..addAll(_chat.items.map((m) => m.id));
      });

  /// ADMIN: TANLANGAN xabarlarni butunlay o'chiradi.
  Future<void> _deleteSelected() async {
    if (!_canDelete || _selected.isEmpty) return;
    final n = _selected.length;
    final ok = await _confirm(n == 1
        ? 'Xabar butunlay o\'chirilsinmi?'
        : '$n ta xabar butunlay o\'chirilsinmi?');
    if (ok != true) return;
    final ids = _selected.toList();
    final err = await _chat.removeMessages(ids);
    if (!mounted) return;
    _clearSelection();
    if (err != null) _snack(err);
  }

  /// Ha/Yo'q so'raydigan oyna.
  Future<bool?> _confirm(String text) async {
    if (!_canDelete) return false;
    final ok = await showDialog<bool>(
      context: context,
      barrierColor: Colors.black54,
      builder: (ctx) => Dialog(
        backgroundColor: Colors.transparent,
        insetPadding: const EdgeInsets.symmetric(horizontal: 28),
        child: Glass(
          borderRadius: 22,
          padding: const EdgeInsets.fromLTRB(22, 22, 22, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                text,
                textAlign: TextAlign.center,
                style: const TextStyle(
                    color: Colors.white, fontSize: 15, height: 1.4),
              ),
              const SizedBox(height: 18),
              Row(
                children: [
                  Expanded(
                    child: OutlinedButton(
                      onPressed: () => Navigator.of(ctx).pop(false),
                      child: const Text('Yo\'q'),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: FilledButton(
                      onPressed: () => Navigator.of(ctx).pop(true),
                      style: FilledButton.styleFrom(
                          backgroundColor: Colors.red.shade600),
                      child: const Text('Ha'),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
    return ok;
  }

  // ══════════════════════════════════════════════════════════
  //  STIKER / GIF (Telegram paneli)
  // ══════════════════════════════════════════════════════════

  Future<void> _sendSticker(TgDoc d) async {
    final err = await _chat.send('', mediaFile: d.ref, mediaType: 'sticker');
    if (err != null && mounted) _snack(err);
    _toBottom();
  }

  /// GIF — Telegram'dagi tayyor fayl kanalga joylanadi (qayta
  /// yuklanmaydi), xabarga esa uning nomi yoziladi.
  Future<void> _sendGif(TgDoc d) async {
    if (_sending) return;
    final me = AuthService.instance.user?.id ?? 0;
    final name = 'chat_${me}_${DateTime.now().millisecondsSinceEpoch}.mp4';
    setState(() => _sending = true);
    var err = await TelegramService.instance.sendGif(d.id, name);
    err ??= await _chat.send('', mediaFile: name, mediaType: 'gif');
    if (!mounted) return;
    setState(() => _sending = false);
    if (err != null) _snack(err);
    _toBottom();
  }

  // ══════════════════════════════════════════════════════════
  //  DUMALOQ VIDEO XABAR (Telegram'dagidek)
  // ══════════════════════════════════════════════════════════

  static const _roundMax = Duration(seconds: 60);

  /// Dumaloq video yozish (Telegram `InstantCameraView` kabi): doira
  /// barmoq bosilishi BILAN chiqadi, kamera esa uning ichida ochiladi.
  Future<bool> _startRound() async {
    if (_roundRec || _recording || _uploading) return false;
    await VoicePlayer.instance.stop();
    HapticFeedback.lightImpact();
    _recLen = Duration.zero;
    _recStart = DateTime.now();
    setState(() {
      _roundRec = true;
      _roundSent = false;
      _cam = null;
    });
    try {
      final cams = await availableCameras();
      if (cams.isEmpty) throw 'kamera topilmadi';
      final cam = cams.firstWhere(
          (c) => c.lensDirection == CameraLensDirection.front,
          orElse: () => cams.first);
      final c = CameraController(cam, ResolutionPreset.medium,
          enableAudio: true);
      await c.initialize();
      // Kamera ochilguncha barmoq qo'yib yuborilgan (yoki bekor
      // qilingan) — yozish boshlanmaydi.
      if (!mounted || !_roundRec) {
        await c.dispose();
        return false;
      }
      setState(() => _cam = c);
      await c.startVideoRecording();
      _recTimer?.cancel();
      _recTimer = Timer.periodic(const Duration(milliseconds: 100), (_) {
        if (!mounted) return;
        setState(() => _recLen += const Duration(milliseconds: 100));
        if (_recLen >= _roundMax) _stopRec(true);
      });
      return true;
    } catch (e) {
      if (mounted) {
        setState(() {
          _roundRec = false;
          _cam = null;
        });
        _snack('Kamera ochilmadi: $e');
      }
      return false;
    }
  }

  /// Chiroq (Telegram `flashButton`): orqa kamerada — fonar, old
  /// kamerada — ekranning o'zi oq yonadi (`FlashViews`).
  bool _roundFlash = false;

  Future<void> _toggleFlash() async {
    final c = _cam;
    final on = !_roundFlash;
    setState(() => _roundFlash = on);
    if (c == null) return;
    if (c.description.lensDirection == CameraLensDirection.back) {
      try {
        await c.setFlashMode(on ? FlashMode.torch : FlashMode.off);
      } catch (_) {}
    }
  }

  /// Old va orqa kamera orasida almashtirish (yozish to'xtamaydi).
  Future<void> _switchCamera() async {
    final c = _cam;
    if (c == null) return;
    try {
      final cams = await availableCameras();
      final now = c.description.lensDirection;
      final other = cams.firstWhere((x) => x.lensDirection != now,
          orElse: () => c.description);
      if (other == c.description) return;
      await c.setDescription(other);
      if (mounted) setState(() {});
    } catch (_) {
      // Qurilma yozish paytida almashtirishni qo'llamaydi.
    }
  }

  Future<void> _stopRound({required bool send}) async {
    if (!_roundRec) return;
    final c = _cam;
    _locked = false;
    _dragX = 0;
    _recTimer?.cancel();
    _recTimer = null;
    final len = _recLen;
    // Doira yopilish animatsiyasi (yuborilsa — xabar tomon "uchadi").
    setState(() {
      _roundRec = false;
      _roundSent = send && len.inMilliseconds >= 1000;
      _roundFlash = false;
    });
    if (c == null) return;
    XFile? f;
    try {
      if (c.value.isRecordingVideo) f = await c.stopVideoRecording();
    } catch (_) {}
    // Animatsiya tugaguncha kamera ko'rinib tursin.
    await Future<void>.delayed(const Duration(milliseconds: 240));
    if (mounted && identical(_cam, c)) setState(() => _cam = null);
    await c.dispose();
    if (f == null) return;
    final file = File(f.path);
    if (!send || len.inMilliseconds < 1000) {
      try {
        await file.delete();
      } catch (_) {}
      if (send && mounted) _snack('Juda qisqa');
      return;
    }
    await _uploadAndSend(
      file: file,
      ext: 'mp4',
      type: 'round',
      contentType: 'video/mp4',
      ms: len.inMilliseconds,
      cleanup: () async {
        try {
          await file.delete();
        } catch (_) {}
      },
    );
  }

  // ══════════════════════════════════════════════════════════
  //  BIRIKTIRISH (Telegram'dagidek, `tg_attach_sheet.dart`)
  // ══════════════════════════════════════════════════════════

  static const _mimes = {
    'jpg': 'image/jpeg', 'jpeg': 'image/jpeg', 'png': 'image/png',
    'webp': 'image/webp', 'gif': 'image/gif', 'heic': 'image/heic',
    'mp4': 'video/mp4', 'mov': 'video/quicktime', 'mkv': 'video/x-matroska',
    'webm': 'video/webm', '3gp': 'video/3gpp',
    'mp3': 'audio/mpeg', 'm4a': 'audio/mp4', 'ogg': 'audio/ogg',
    'wav': 'audio/wav', 'flac': 'audio/flac',
    'pdf': 'application/pdf', 'zip': 'application/zip',
    'txt': 'text/plain', 'apk': 'application/vnd.android.package-archive',
    'doc': 'application/msword', 'xls': 'application/vnd.ms-excel',
    'docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    'xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  };

  Future<void> _openAttach() async {
    _focus.unfocus();
    final r = await showTgAttachSheet(context);
    if (r == null || !mounted) return;
    for (var i = 0; i < r.items.length; i++) {
      final it = r.items[i];
      final dot = it.name.lastIndexOf('.');
      var ext = dot > 0 ? it.name.substring(dot + 1).toLowerCase() : '';
      if (ext.isEmpty || ext.length > 5 ||
          !RegExp(r'^[a-z0-9]+$').hasMatch(ext)) {
        ext = it.type == 'video' ? 'mp4' : (it.type == 'image' ? 'jpg' : 'bin');
      }
      final ct = _mimes[ext] ??
          (it.type == 'video'
              ? 'video/mp4'
              : it.type == 'image'
                  ? 'image/jpeg'
                  : 'application/octet-stream');
      // Kamera va fayl tanlagichi faylni ilovaning keshiga nusxalaydi —
      // yuborilgach o'chiriladi. Galereyadagi ASL faylga tegilmaydi.
      final temp = it.file.path.contains('/cache/');
      await _uploadAndSend(
        file: it.file,
        ext: ext,
        type: it.type,
        contentType: ct,
        ms: it.durationMs,
        // Fayl xabarida matn o'rnida uning nomi; izoh — birinchisiga.
        body: it.type == 'file' ? it.name : (i == 0 ? r.caption : ''),
        cleanup: temp
            ? () async {
                try {
                  await it.file.delete();
                } catch (_) {}
              }
            : null,
      );
    }
  }

  /// Yozish tugmasi: ovoz yoki dumaloq video boshlanadi.
  Future<bool> _startRec(TgRecMode mode) async {
    _focus.unfocus();
    if (mode == TgRecMode.video) return _startRound();
    await _startRecording();
    if (_recording) HapticFeedback.lightImpact();
    return _recording;
  }

  void _stopRec(bool send) {
    setState(() {
      _locked = false;
      _dragX = 0;
    });
    if (_roundRec) {
      unawaited(_stopRound(send: send));
    } else {
      unawaited(_stopRecording(send: send));
    }
  }

  /// Xabarning qisqa matni (iqtibos va javob qatori uchun).
  String _snippet(ChatMessage m) {
    final body = tgSplitReply(m.body).$2;
    if (m.hasMedia) {
      final label = switch (m.mediaType) {
        'voice' => 'Ovozli xabar',
        'round' => 'Video xabar',
        'sticker' => 'Stiker',
        'gif' => 'GIF',
        'file' => m.body.isEmpty ? 'Fayl' : m.body,
        'video' => 'Video',
        _ => 'Rasm',
      };
      if (m.mediaType == 'file' || m.isVoice || body.isEmpty) return label;
      return '$label, ${plainEmojiText(body)}';
    }
    return plainEmojiText(body).replaceAll('\n', ' ');
  }

  List<ChatMessage>? _byIdSrc;
  Map<String, ChatMessage> _byIdMap = const {};
  Map<String, ChatMessage> _byId(List<ChatMessage> items) {
    if (!identical(items, _byIdSrc) || _byIdMap.length != items.length) {
      _byIdSrc = items;
      _byIdMap = {for (final x in items) x.id: x};
    }
    return _byIdMap;
  }

  bool _isMine(ChatMessage m) =>
      _chat.isAdminView ? m.fromAdmin : !m.fromAdmin;

  String _nameOf(ChatMessage m) =>
      _isMine(m) ? 'Siz' : (widget.title.isEmpty ? 'Admin' : widget.title);

  void _startReply(ChatMessage m) {
    if (m.pending) return;
    HapticFeedback.selectionClick();
    setState(() => _replyTo = m);
    _focus.requestFocus();
  }

  /// Iqtibos bosildi — asl xabarga o'tiladi va u bir zum yoritiladi.
  Future<void> _goTo(String id) async {
    final items = _chat.items;
    final i = items.indexWhere((x) => x.id == id);
    if (i < 0) {
      _snack('Asl xabar topilmadi');
      return;
    }
    var ctx = _keys[id]?.currentContext;
    if (ctx == null && _scroll.hasClients) {
      // Hali qurilmagan — taxminiy joyga sakrab, so'ng aniq joylanadi.
      final p = _scroll.position;
      _scroll.jumpTo((p.maxScrollExtent * i / items.length)
          .clamp(0.0, p.maxScrollExtent));
      await Future<void>.delayed(const Duration(milliseconds: 60));
      ctx = _keys[id]?.currentContext;
    }
    if (ctx != null && ctx.mounted) {
      await Scrollable.ensureVisible(ctx,
          alignment: 0.35,
          duration: const Duration(milliseconds: 300),
          curve: Curves.easeOutCubic);
    }
    if (!mounted) return;
    setState(() => _flash = id);
    await Future<void>.delayed(const Duration(milliseconds: 1200));
    if (mounted && _flash == id) setState(() => _flash = null);
  }

  /// Xabarni bosib turish menyusi (Telegram `ActionBarPopupWindow`).
  Future<void> _menu(ChatMessage m, Offset at) async {
    if (_selecting) return;
    HapticFeedback.mediumImpact();
    final text = tgSplitReply(m.body).$2;
    final canCopy = !m.hasMedia && text.isNotEmpty;
    final overlay =
        Overlay.of(context).context.findRenderObject() as RenderBox;
    PopupMenuItem<String> item(String v, IconData icon, String label,
            {Color color = Colors.white}) =>
        PopupMenuItem<String>(
          value: v,
          height: 48,
          child: Row(
            children: [
              Icon(icon, size: 22, color: color.withValues(alpha: 0.8)),
              const SizedBox(width: 18),
              Text(label, style: TextStyle(color: color, fontSize: 16)),
            ],
          ),
        );
    final v = await showMenu<String>(
      context: context,
      color: const Color(0xFF26272B),
      shape:
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      position: RelativeRect.fromRect(
          at & const Size(1, 1), Offset.zero & overlay.size),
      items: [
        if (!m.pending) item('reply', Icons.reply_rounded, 'Javob berish'),
        if (canCopy) item('copy', Icons.copy_rounded, 'Nusxa olish'),
        if (_canDelete) ...[
          item('select', Icons.check_circle_outline_rounded, 'Tanlash'),
          item('delete', Icons.delete_outline_rounded, 'O\'chirish',
              color: const Color(0xFFFF5A5A)),
        ],
      ],
    );
    if (!mounted || v == null) return;
    switch (v) {
      case 'reply':
        _startReply(m);
      case 'copy':
        await Clipboard.setData(ClipboardData(text: plainEmojiText(text)));
        _snack('Nusxa olindi');
      case 'select':
        _toggleSelect(m);
      case 'delete':
        _toggleSelect(m);
        await _deleteSelected();
    }
  }

  Future<void> _send() async {
    if (_sending) return;
    final text = _input.encoded.trim();
    if (text.isEmpty) return;
    setState(() => _sending = true);
    final reply = _replyTo;
    final err = await _chat.send(tgWithReply(reply?.id, text));
    if (!mounted) return;
    if (err == null) _replyTo = null;
    if (!mounted) return;
    setState(() => _sending = false);
    if (err != null) {
      _snack(err);
      return;
    }
    _input.clear();
    setState(() {});
    _toBottom();
  }

  @override
  Widget build(BuildContext context) {
    return AppBackground(
      child: PopScope(
        // Tanlash rejimi ochiq bo'lsa "orqaga" avval TANLOVNI
        // bekor qiladi — ekran yopilib ketmaydi.
        canPop: !_selecting,
        onPopInvokedWithResult: (didPop, _) {
          if (!didPop && _selecting) _clearSelection();
        },
        child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: _selecting ? _selectionBar() : _normalBar(),
        body: SafeArea(
          top: false,
          child: Stack(
            children: [
              Column(
                children: [
                  Expanded(
                    child: Stack(
                      children: [
                        Positioned.fill(
                          child: AnimatedBuilder(
                            animation: _chat,
                            builder: (context, _) => _body(),
                          ),
                        ),
                        // Dumaloq video yozilayotganda — kamera doirasi
                        // (Telegram `InstantCameraView` kabi) FAQAT xabarlar
                        // ustida: pastdagi yozish paneli (vaqt, "bekor
                        // qilish uchun suring", tugma) ko'rinib turadi.
                        // Pastga tushish tugmasi (Telegram `pagedownButton`):
                        // ro'yxat yuqoriga surilganda o'ngda pastda chiqadi.
                        Positioned(
                          right: 10,
                          bottom: 10,
                          child: AnimatedScale(
                            scale: _showDown ? 1 : 0,
                            duration: const Duration(milliseconds: 200),
                            curve: Curves.easeOutBack,
                            child: GestureDetector(
                              onTap: () {
                                if (!_scroll.hasClients) return;
                                _scroll.animateTo(
                                    _scroll.position.maxScrollExtent,
                                    duration:
                                        const Duration(milliseconds: 300),
                                    curve: Curves.easeOutCubic);
                              },
                              child: Container(
                                width: 42,
                                height: 42,
                                decoration: BoxDecoration(
                                  color: const Color(0xFF26272B),
                                  shape: BoxShape.circle,
                                  boxShadow: const [
                                    BoxShadow(
                                        color: Colors.black38,
                                        blurRadius: 6,
                                        offset: Offset(0, 2)),
                                  ],
                                ),
                                child: const Icon(
                                    Icons.keyboard_arrow_down_rounded,
                                    color: Colors.white,
                                    size: 28),
                              ),
                            ),
                          ),
                        ),
                        TgRoundOverlay(
                          camera: _cam,
                          active: _roundRec,
                          sent: _roundSent,
                          length: _recLen,
                          max: _roundMax,
                          flash: _roundFlash,
                          onSwitchCamera: _switchCamera,
                          onFlash: _toggleFlash,
                        ),
                      ],
                    ),
                  ),
                  _composer(),
                ],
              ),
            ],
          ),
        ),
      ),
      ),
    );
  }

  /// ── TANLASH PANELI ──────────────────────────────────────
  ///
  /// Chapda — tanlovni bekor qilish, o'rtada nechtasi
  /// tanlangani, O'NG YUQORIDA esa chiqindi tugmasi
  /// (foydalanuvchi talabi). Yonida "hammasini tanlash".
  PreferredSizeWidget _selectionBar() {
    final all = _chat.items.isNotEmpty &&
        _selected.length >= _chat.items.length;
    return AppBar(
      backgroundColor: Colors.transparent,
      elevation: 0,
      iconTheme: const IconThemeData(color: Colors.white),
      titleSpacing: 0,
      leading: IconButton(
        icon: const Icon(Icons.close_rounded, color: Colors.white),
        onPressed: _clearSelection,
      ),
      title: Text(
        '${_selected.length} ta tanlandi',
        style: const TextStyle(color: Colors.white, fontSize: 17),
      ),
      actions: [
        IconButton(
          tooltip: all ? 'Tanlovni olish' : 'Hammasini tanlash',
          icon: Icon(
            all ? Icons.deselect_rounded : Icons.select_all_rounded,
            color: Colors.white,
          ),
          onPressed: all ? _clearSelection : _selectAll,
        ),
        IconButton(
          tooltip: 'O\'chirish',
          icon: const Icon(Icons.delete_outline_rounded),
          color: Colors.red.shade400,
          onPressed: _deleteSelected,
        ),
        const SizedBox(width: 4),
      ],
    );
  }

  PreferredSizeWidget _normalBar() {
    return AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          titleSpacing: 0,
          title: Row(
            children: [
              // ── RASMGA BOSSA — PROFIL ─────────────────────────
              //
              // TALAB: "chatdagi profil rasmi ustiga bosganda
              // profili ochilib profil to'liq ko'rinsin".
              // ── `ChatAvatarContainer`: 42 dp rasm, nom (18, qalin) va
              // ostida kulrang holat qatori (15).
              GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: widget.userId == null
                    ? null
                    : () => Navigator.of(context).push(
                          MaterialPageRoute<void>(
                            builder: (_) =>
                                PublicProfileScreen(userId: widget.userId!),
                          ),
                        ),
                child: widget.userId != null
                    ? _TitleAvatar(
                        url: widget.photoUrl, name: widget.title, size: 42)
                    // Foydalanuvchi tomonida — yordam xizmati belgisi.
                    : ClipOval(
                        child: Container(
                          width: 42,
                          height: 42,
                          color: Colors.black,
                          padding: const EdgeInsets.all(7),
                          child: Image.asset('assets/aru-mark.png'),
                        ),
                      ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 18,
                          fontWeight: FontWeight.w600),
                    ),
                    const SizedBox(height: 1),
                    Text(
                      widget.userId != null
                          ? 'ID: ${widget.userId}'
                          : 'yordam xizmati',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                          color: Colors.white.withValues(alpha: 0.55),
                          fontSize: 14),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
  }

  Widget _body() {
    if (_chat.isLoading) {
      return Center(
        child: CircularProgressIndicator(
          strokeWidth: 2,
          valueColor: AlwaysStoppedAnimation(AppColors.accent),
        ),
      );
    }
    final items = _chat.items;
    if (items.isEmpty && !_uploading) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.support_agent_rounded,
                  size: 54, color: Colors.white.withValues(alpha: 0.2)),
              const SizedBox(height: 14),
              Text(
                _chat.error ??
                    (_chat.isAdminView
                        ? 'Bu odam hali yozmagan'
                        : 'Savolingiz bormi? Yozing — admin javob beradi.'),
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.45),
                  fontSize: 13.5,
                  height: 1.5,
                ),
              ),
            ],
          ),
        ),
      );
    }

    return NotificationListener<ScrollNotification>(
      // ── PASTDAN TORTIB YANGILASH ────────────────────────
      //
      // TALAB (foydalanuvchi): "chatdagi xabarlarni yuqoriga
      // ko'tarsa pastda aylanadigan narsa chiqsin va serverdan
      // chatga xabar kelgan-kelmaganini tekshirsin".
      //
      // Ro'yxatda eng yangi xabar PASTDA turadi, ya'ni "yuqoriga
      // ko'tarish" — ro'yxatning OXIRIDAN tashqariga chiqish.
      // Shu sabab oddiy `RefreshIndicator` yaramaydi (u faqat
      // tepadan ishlaydi) va tekshiruv qo'lda qilinadi.
      onNotification: _onScroll,
      child: ListView.builder(
      controller: _scroll,
      physics: const BouncingScrollPhysics(
          parent: AlwaysScrollableScrollPhysics()),
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
      // Oxirida: yuklanayotgan fayl va (kerak bo'lsa) aylana.
      itemCount: items.length + (_uploading ? 1 : 0) + (_pullBusy ? 1 : 0),
      itemBuilder: (context, i) {
        if (i >= items.length + (_uploading ? 1 : 0)) {
          return const Padding(
            padding: EdgeInsets.symmetric(vertical: 14),
            child: Center(
              child: SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(
                    strokeWidth: 2, color: Colors.white54),
              ),
            ),
          );
        }
        if (i >= items.length) {
          return _UploadingBubble(
            type: _upType,
            path: _upPath,
            progress: _upProgress,
            ms: _upMs,
          );
        }
        final m = items[i];
        final byId = _byId(items);
        // O'z xabarim o'ngda. Admin ekranida "o'ziniki" — admin
        // yozganlari; foydalanuvchi ekranida esa aksincha.
        final mine = _chat.isAdminView ? m.fromAdmin : !m.fromAdmin;
        // ── SUHBATDOSH RASMI XABAR YONIDA ──────────────────
        //
        // TALAB (foydalanuvchi): "admin bilan gaplashadigan chat
        // ichida izohlardagidek adminga foydalanuvchi profili
        // ko'rinib tursin, ustiga bosib profilni ko'rish mumkin
        // bo'lsin".
        //
        // Rasm faqat SUHBATDOSHNING xabari yonida turadi (o'z
        // xabarining yonida o'z rasmini ko'rsatishning ma'nosi
        // yo'q — Telegram ham shunday qiladi).
        //
        // Ketma-ket kelgan xabarlarda rasm faqat OXIRGISIDA
        // chiziladi: aks holda bir xil rasm ustma-ust takrorlanib,
        // ro'yxat g'ijimlanib ketardi.
        final next = i + 1 < items.length ? items[i + 1] : null;
        final prev = i > 0 ? items[i - 1] : null;
        final lastOfGroup = next == null ||
            next.fromAdmin != m.fromAdmin ||
            !tgSameDay(next.createdAt, m.createdAt);
        // Kun almashganda — sana ajratgichi (`ChatActionCell`).
        final newDay =
            prev == null || !tgSameDay(prev.createdAt, m.createdAt);
        final topNear = !newDay && prev.fromAdmin == m.fromAdmin;
        final bubble = _Bubble(
          topNear: topNear,
          bottomNear: !lastOfGroup,
          thumbs: _thumbs,
          message: m,
          mine: mine,
          avatarUrl: mine ? '' : widget.photoUrl,
          avatarName: mine ? '' : widget.title,
          showAvatar: !mine && lastOfGroup,
          onAvatarTap: widget.userId == null
              ? null
              : () => Navigator.of(context).push(
                    MaterialPageRoute<void>(
                      builder: (_) =>
                          PublicProfileScreen(userId: widget.userId!),
                    ),
                  ),
          // Admin uzoq bosib TANLAYDI, keyin qolganlarini oddiy
          // bosib qo'shadi. Foydalanuvchida ikkovi ham ishlamaydi.
          onLongPressAt: (at) => _menu(m, at),
          onTap: _selecting ? () => _toggleSelect(m) : null,
          selected: _selected.contains(m.id) || _flash == m.id,
          quote: () {
            final rid = tgSplitReply(m.body).$1;
            if (rid == null) return null;
            final orig = byId[rid];
            final mineColor = Colors.white;
            return TgReplyQuote(
              name: orig == null ? 'Xabar' : _nameOf(orig),
              text: orig == null ? 'o\'chirilgan' : _snippet(orig),
              color: mine ? mineColor : AppColors.accent2,
              background: Colors.white.withValues(alpha: 0.08),
              onTap: () => _goTo(rid),
            );
          }(),
          selecting: _selecting,
          onOpenMedia: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => MediaViewScreen(
                url: m.mediaUrl,
                type: m.mediaType,
              ),
            ),
          ),
        );
        final row = KeyedSubtree(
          key: _keys[m.id] ??= GlobalKey(),
          child: TgSwipeReply(
            onReply: _selecting || m.pending ? null : () => _startReply(m),
            child: bubble,
          ),
        );
        if (!newDay) return row;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [TgDateChip(tgDayLabel(m.createdAt)), row],
        );
      },
      ),
    );
  }

  // ══════════════════════════════════════════════════════════
  //  YOZISH QATORI — TELEGRAM'DAGIDEK
  // ══════════════════════════════════════════════════════════
  //
  // TALAB (foydalanuvchi): "support chatga emoji, GIF, stiker, matn
  // yozadigan, fayl yuboradigan, ovozli xabar va dumaloq xabar
  // yuboradigan oynani qo'sh — huddi Telegram'niki bilan bir xil".
  //
  //   [🙂  Xabar            📎]  (🎤 / ⏺ / ➤)
  //
  // 🙂 — Emoji / GIF / Stikerlar paneli (`tg_composer.dart`);
  // 📎 — rasm yoki video; o'ngdagi tugma — `tg_record_button.dart`.
  //
  // Klaviatura joyini `Scaffold` o'zi ochadi (bu yerda qo'shimcha
  // bo'shliq qo'yilmaydi — ilgari qator ikki barobar sakrardi).
  Widget _composer() {
    final active = _recording || _roundRec;
    return Container(
      decoration: BoxDecoration(
        color: AppColors.surface,
        border: Border(
          top: BorderSide(color: Colors.white.withValues(alpha: 0.08)),
        ),
      ),
      child: TgInputArea(
        controller: _input,
        focus: _focus,
        onSticker: _sendSticker,
        onGif: _sendGif,
        row: (context, emojiButton) => Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            // Javob qatori (Telegram `replyLine`): yozish maydonining
            // tepasida, ✕ bilan bekor qilinadi.
            AnimatedSize(
              duration: const Duration(milliseconds: 180),
              curve: Curves.easeOutCubic,
              child: _replyTo == null || active
                  ? const SizedBox(width: double.infinity)
                  : TgReplyBar(
                      name: _nameOf(_replyTo!),
                      text: _snippet(_replyTo!),
                      color: AppColors.accent2,
                      onClose: () => setState(() => _replyTo = null),
                    ),
            ),
            Padding(
          padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: active ? _recordingInfo() : _field(emojiButton),
              ),
              const SizedBox(width: 8),
              TgRecordButton(
                hasText: !_input.isBlank && !active,
                busy: _sending,
                locked: _locked,
                onSend: _send,
                onStart: _startRec,
                onStop: _stopRec,
                onLock: () => setState(() => _locked = true),
                onDrag: (dx) => setState(() => _dragX = dx),
                amplitude: _amp,
              ),
            ],
          ),
        ),
          ],
        ),
      ),
    );
  }

  /// Oddiy holat: 🙂, matn maydoni va 📎.
  Widget _field(Widget emojiButton) {
    return Container(
      constraints: const BoxConstraints(minHeight: 44),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.07),
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: Colors.white.withValues(alpha: 0.12)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          emojiButton,
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 11),
              child: TextField(
                controller: _input,
                focusNode: _focus,
                minLines: 1,
                maxLines: 6,
                maxLength: 2000,
                textCapitalization: TextCapitalization.sentences,
                style: const TextStyle(color: Colors.white, fontSize: 15),
                cursorColor: AppColors.accent,
                decoration: InputDecoration(
                  counterText: '',
                  isDense: true,
                  contentPadding: EdgeInsets.zero,
                  hintText: 'Xabar',
                  hintStyle: TextStyle(
                      color: Colors.white.withValues(alpha: 0.38),
                      fontSize: 15),
                  border: InputBorder.none,
                ),
                onChanged: (_) => setState(() {}),
              ),
            ),
          ),
          // Telegram'dagidek: matn yozilayotganda 📎 ham turadi.
          // 📎 — Telegram'dagidek ilova ichidagi galereya / fayl oynasi.
          GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: _uploading ? null : _openAttach,
            child: SizedBox(
              width: 42,
              height: 44,
              child: Transform.rotate(
                angle: 0.6,
                child: Icon(Icons.attach_file_rounded,
                    size: 23,
                    color: Colors.white
                        .withValues(alpha: _uploading ? 0.25 : 0.55)),
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// Yozish paytidagi qator (`RecordDot`, `TimerView`,
  /// `SlideTextView`): ● 0:03,45 va "‹ Bekor qilish uchun suring"
  /// (yaltirab turadi, barmoq bilan siljiydi); qulflanganda —
  /// "BEKOR QILISH".
  Widget _recordingInfo() {
    return SizedBox(
      height: 48,
      child: Row(
        children: [
          const SizedBox(width: 12),
          const _BlinkDot(),
          const SizedBox(width: 10),
          _RecTimer(start: _recStart),
          Expanded(
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 200),
              child: _locked
                  ? Center(
                      key: const ValueKey('cancel'),
                      child: TextButton(
                        onPressed: () => _stopRec(false),
                        child: const Text('BEKOR QILISH',
                            style: TextStyle(
                                color: AppColors.accent2,
                                fontSize: 15,
                                fontWeight: FontWeight.w700)),
                      ),
                    )
                  : Transform.translate(
                      key: const ValueKey('slide'),
                      offset: Offset(_dragX, 0),
                      child: Opacity(
                        opacity: (1 + _dragX / 60).clamp(0.0, 1.0),
                        child: const Center(child: _SlideToCancel()),
                      ),
                    ),
            ),
          ),
        ],
      ),
    );
  }

}

/// Yozish paytidagi qizil nuqta (`RecordDot`): 600 ms da so'nadi va
/// yana yonadi.
class _BlinkDot extends StatefulWidget {
  const _BlinkDot();

  @override
  State<_BlinkDot> createState() => _BlinkDotState();
}

class _BlinkDotState extends State<_BlinkDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _a = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 600))
    ..repeat(reverse: true);

  @override
  void dispose() {
    _a.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => FadeTransition(
        opacity: Tween(begin: 1.0, end: 0.0).animate(_a),
        child: Container(
          width: 10,
          height: 10,
          decoration: const BoxDecoration(
              color: Color(0xFFE5484D), shape: BoxShape.circle),
        ),
      );
}

/// `TimerView`: "0:03,45" (15, soniyaning yuzdan biri bilan).
class _RecTimer extends StatefulWidget {
  final DateTime start;
  const _RecTimer({required this.start});

  @override
  State<_RecTimer> createState() => _RecTimerState();
}

class _RecTimerState extends State<_RecTimer>
    with SingleTickerProviderStateMixin {
  late final Ticker _t = createTicker((_) => setState(() {}))..start();

  @override
  void dispose() {
    _t.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final d = DateTime.now().difference(widget.start);
    final ms = d.inMilliseconds;
    final m = ms ~/ 60000;
    final sec = (ms ~/ 1000) % 60;
    final cs = (ms % 1000) ~/ 10;
    return Text(
      '$m:${sec.toString().padLeft(2, '0')},${cs.toString().padLeft(2, '0')}',
      style: const TextStyle(
          color: Colors.white,
          fontSize: 15,
          fontFeatures: [FontFeature.tabularFigures()]),
    );
  }
}

/// `SlideTextView`: "‹ Bekor qilish uchun suring" — kulrang, ustidan
/// yorug' chiziq yurib turadi.
class _SlideToCancel extends StatefulWidget {
  const _SlideToCancel();

  @override
  State<_SlideToCancel> createState() => _SlideToCancelState();
}

class _SlideToCancelState extends State<_SlideToCancel>
    with SingleTickerProviderStateMixin {
  late final AnimationController _a = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 1500))
    ..repeat();

  @override
  void dispose() {
    _a.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    const gray = Color(0xFF8A939D);
    return AnimatedBuilder(
      animation: _a,
      builder: (context, child) => ShaderMask(
        blendMode: BlendMode.srcIn,
        shaderCallback: (r) {
          // Yorug'lik o'ngdan chapga yuradi (strelka yo'nalishida).
          final x = 1.4 - 1.8 * _a.value;
          return LinearGradient(
            colors: const [gray, Colors.white, gray],
            stops: [
              (x - 0.2).clamp(0.0, 1.0),
              x.clamp(0.0, 1.0),
              (x + 0.2).clamp(0.0, 1.0),
            ],
          ).createShader(r);
        },
        child: child,
      ),
      child: const Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.chevron_left_rounded, size: 20, color: gray),
          Flexible(
            child: Text(
              'Bekor qilish uchun suring',
              maxLines: 1,
              overflow: TextOverflow.fade,
              softWrap: false,
              style: TextStyle(color: gray, fontSize: 15),
            ),
          ),
        ],
      ),
    );
  }
}

/// Sarlavhadagi kichik rasm.
class _TitleAvatar extends StatelessWidget {
  final String url;
  final String name;
  final double size;

  const _TitleAvatar({
    required this.url,
    required this.name,
    this.size = 34,
  });

  @override
  Widget build(BuildContext context) {
    final letter = name.trim().isEmpty
        ? '?'
        : name.trim().characters.first.toUpperCase();
    final fallback = Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      color: AppColors.cardAlt,
      child: Text(
        letter,
        style: TextStyle(
          color: Colors.white.withValues(alpha: 0.7),
          fontSize: size * 0.42,
          fontWeight: FontWeight.w800,
        ),
      ),
    );
    return ClipOval(
      child: SizedBox(
        width: size,
        height: size,
        child: url.isEmpty
            ? fallback
            : CachedNetworkImage(
                cacheManager: AppImageCache.manager,
                imageUrl: url,
                fit: BoxFit.cover,
                memCacheWidth: (size * 3).round(),
                placeholder: (_, __) => fallback,
                errorWidget: (_, __, ___) => fallback,
              ),
      ),
    );
  }
}

/// Bitta xabar puffagi.
class _Bubble extends StatelessWidget {
  final ChatMessage message;
  final bool mine;

  /// Suhbatdoshning rasmi (o'z xabarida bo'sh).
  final String avatarUrl;
  final String avatarName;

  /// Guruhdagi OXIRGI xabarmi — rasm faqat shunda chiziladi.
  final bool showAvatar;
  final VoidCallback? onAvatarTap;

  /// Admin uchun — uzoq bosilganda tanlash boshlanadi.
  /// Bosib turildi (menyu shu nuqtada ochiladi).
  final ValueChanged<Offset>? onLongPressAt;

  /// Javob iqtibosi (bo'lsa).
  final Widget? quote;

  /// Tanlash rejimida bosish tanlaydi, oddiy holatda esa
  /// rasm/video ochiladi.
  final VoidCallback? onTap;
  final VoidCallback onOpenMedia;

  /// Videoning boshidagi kadrni yasovchi (ekranga tegishli).
  final ChatVideoThumb thumbs;

  /// Shu xabar hozir tanlanganmi.
  final bool selected;

  /// Umuman tanlash rejimi ochiqmi (bitta bo'lsa ham).
  final bool selecting;

  /// Tepada/pastda shu yuboruvchining (o'sha kuni) boshqa xabari bor —
  /// Telegram'dagidek o'sha tomondagi burchak kichik, dum faqat
  /// guruhning OXIRGI xabarida.
  final bool topNear;
  final bool bottomNear;

  const _Bubble({
    required this.message,
    required this.mine,
    required this.onOpenMedia,
    required this.thumbs,
    this.avatarUrl = '',
    this.avatarName = '',
    this.showAvatar = false,
    this.onAvatarTap,
    this.onLongPressAt,
    this.quote,
    this.onTap,
    this.selected = false,
    this.selecting = false,
    this.topNear = false,
    this.bottomNear = false,
  });

  @override
  Widget build(BuildContext context) {
    final m = message;
    // Stiker, GIF va dumaloq video — pufaksiz (Telegram'dagidek).
    final bare = m.hasMedia && m.isInline && m.body.isEmpty && quote == null;
    // ── TANLANGAN XABAR AJRALIB TURADI ────────────────────────
    //
    // Butun qator (rasm bilan birga) bo'yaladi — Telegram ham
    // shunday qiladi, ya'ni nimani tanlagani bir qarashda
    // ko'rinadi.
    return Container(
      color: selected
          ? AppColors.accent.withValues(alpha: 0.16)
          : Colors.transparent,
      // Guruh ichida 2 dp, guruhlar orasida 8 dp (Telegram).
      padding: EdgeInsets.only(bottom: bottomNear ? 2 : 8),
      child: Row(
        mainAxisAlignment:
            mine ? MainAxisAlignment.end : MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          if (!mine) ...[
            // Rasm chizilmasa ham JOYI saqlanadi — aks holda
            // guruhdagi xabarlar bir-biriga nisbatan siljib
            // ketardi.
            SizedBox(
              width: 30,
              child: showAvatar
                  ? GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: onAvatarTap,
                      child: _TitleAvatar(
                        url: avatarUrl,
                        name: avatarName,
                        size: 30,
                      ),
                    )
                  : null,
            ),
            const SizedBox(width: 7),
          ],
          Flexible(
            child: GestureDetector(
              onLongPressStart: onLongPressAt == null
                  ? null
                  : (d) => onLongPressAt!(d.globalPosition),
              onTap: onTap,
              behavior: HitTestBehavior.opaque,
              child: ConstrainedBox(
                constraints: BoxConstraints(
                  maxWidth: MediaQuery.sizeOf(context).width * 0.8,
                ),
                child: _content(context, bare),
              ),
            ),
          ),
        ],
      ),
    );
  }

  // ── PUFAKCHA (`ChatMessageCell` + `MessageDrawable`) ──────────
  //
  //   * matn 16, vaqt 12 — matnning oxirgi qatoriga sig'sa o'sha
  //     qatorda o'ngda, sig'masa yangi qatorda;
  //   * rasm/video izohsiz bo'lsa — pufak yo'q, rasmning o'zi
  //     yumaloqlanadi, vaqt rasm ustida qoramtir "tabletka"da;
  //   * stiker, GIF, dumaloq video — pufaksiz, vaqt tabletkada.
  Widget _content(BuildContext context, bool bare) {
    final m = message;
    final shape = TgBubbleShape(
      out: mine,
      tail: !bottomNear,
      topNear: topNear,
      bottomNear: bottomNear,
    );
    final color = mine ? AppColors.accent : const Color(0xFF222326);
    // Javob belgisi (`[re:..]`) matnda ko'rinmaydi — iqtibos bo'lib chiqadi.
    final body = tgSplitReply(m.body).$2;
    final hasText = body.isNotEmpty &&
        m.mediaType != 'file' &&
        !(m.isVoice && tgIsWaveformBody(body));

    if (bare) {
      return Column(
        crossAxisAlignment:
            mine ? CrossAxisAlignment.end : CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          _media(context),
          const SizedBox(height: 4),
          _timePill(),
        ],
      );
    }

    // Izohsiz rasm/video: pufak o'rnida rasmning o'zi (`TYPE_MEDIA`).
    if (m.isViewable && !hasText && quote == null) {
      final media = TgBubbleShape(
        out: mine,
        topNear: topNear,
        bottomNear: bottomNear,
        media: true,
      );
      return Padding(
        padding: EdgeInsets.only(
            left: mine ? 0 : TgBubbleShape.tailInset,
            right: mine ? TgBubbleShape.tailInset : 0),
        child: Stack(
          children: [
            ClipPath(clipper: TgBubbleClip(media), child: _media(context)),
            Positioned(right: 6, bottom: 6, child: _timePill()),
          ],
        ),
      );
    }

    final time = _timeRow(onMedia: false);
    return TgBubble(
      shape: shape,
      color: color,
      padding: m.isViewable
          ? const EdgeInsets.fromLTRB(3, 3, 3, 6)
          : const EdgeInsets.fromLTRB(11, 6, 10, 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (quote != null)
            Padding(
              padding: EdgeInsets.fromLTRB(
                  m.isViewable ? 5 : 0, m.isViewable ? 3 : 0, 0, 2),
              child: quote!,
            ),
          if (m.hasMedia)
            m.isViewable
                ? ClipRRect(
                    borderRadius: BorderRadius.circular(14),
                    child: _media(context))
                : _media(context),
          if (hasText)
            Padding(
              padding: EdgeInsets.fromLTRB(m.isViewable ? 8 : 0,
                  m.isViewable || m.hasMedia ? 6 : 0, m.isViewable ? 7 : 0, 0),
              child: _TextWithTime(
                text: body,
                time: time,
                timeWidth: _timeWidth(),
              ),
            )
          else
            Align(
              alignment: Alignment.centerRight,
              child: Padding(
                padding: EdgeInsets.only(
                    top: 3, right: m.isViewable ? 7 : 0),
                child: time,
              ),
            ),
        ],
      ),
    );
  }

  /// Vaqt va (o'z xabarida) ✓ / ✓✓.
  Widget _timeRow({required bool onMedia}) {
    final m = message;
    final c = onMedia
        ? Colors.white
        : Colors.white.withValues(alpha: mine ? 0.75 : 0.5);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          tgTime(m.createdAt),
          style: TextStyle(color: c, fontSize: 12, height: 1.1),
        ),
        // Belgi FAQAT o'z xabaringizda (suhbatdosh xabari yonida
        // uning "o'qildi" holati ma'nosiz).
        if (mine) ...[
          const SizedBox(width: 3),
          _SendState(pending: m.pending, seen: m.seen),
        ],
      ],
    );
  }

  /// Vaqt egallaydigan kenglik (matn oxirida shuncha joy qoldiriladi).
  double _timeWidth() {
    final tp = TextPainter(
      text: TextSpan(
          text: tgTime(message.createdAt),
          style: const TextStyle(fontSize: 12, height: 1.1)),
      textDirection: TextDirection.ltr,
    )..layout();
    return tp.width + (mine ? 20 : 0) + 8;
  }

  /// Media ustidagi vaqt (`chat_mediaTimeBackground`).
  Widget _timePill() {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: const Color(0x66000000),
        borderRadius: BorderRadius.circular(10),
      ),
      child: _timeRow(onMedia: true),
    );
  }

  /// Rasm yoki videoning kichik ko'rinishi.
  ///
  /// Video uchun qora fon va o'rtada play belgisi — bosilganda
  /// sodda ko'ruvchi ochiladi (`media_view_screen.dart`).
  Widget _media(BuildContext context) {
    final m = message;
    // ── OVOZLI XABAR ────────────────────────────────────────
    //
    // U ko'ruvchida ochilmaydi — xabarning O'ZIDA ijro etiladi
    // (Telegram ham shunday qiladi).
    if (m.isVoice) {
      return _VoiceBubble(
        message: m,
        mine: mine,
        // Tanlash rejimida bosish TANLAYDI, ijro qilmaydi.
        onSelect: selecting ? onTap : null,
      );
    }
    final name = TelegramService.fileNameOf(m.mediaUrl);
    switch (m.mediaType) {
      case 'sticker':
        return TgStickerRefView(ref: name, size: 150);
      case 'gif':
        return TgGifMessage(fileName: name, maxWidth: 220);
      case 'round':
        return _RoundBubble(
          url: m.mediaUrl,
          ms: m.mediaMs,
          onSelect: selecting ? onTap : null,
        );
      case 'file':
        return _FileBubble(
          url: m.mediaUrl,
          name: m.body,
          mine: mine,
          onSelect: selecting ? onTap : null,
        );
    }
    return GestureDetector(
      // Tanlash rejimida rasm/video OCHILMAYDI — bosish tanlaydi.
      // Aks holda tanlayman deb bosgan odam har safar video
      // ko'ruvchiga tushib ketardi.
      onTap: selecting ? onTap : onOpenMedia,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(13),
        child: Container(
          constraints: const BoxConstraints(maxHeight: 240, minWidth: 150),
          color: Colors.black.withValues(alpha: 0.35),
          child: m.isVideo
              ? SizedBox(
                  height: 150,
                  width: 220,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      // ── VIDEONING BOSHIDAGI KADRI ────────
                      //
                      // Serverdan KELMAYDI (foydalanuvchi
                      // talabi: "serverga thumbnail
                      // yuklanmasin") — uni shu telefonning
                      // o'zi yasaydi va o'zida saqlaydi
                      // (`chat_video_thumb.dart`).
                      //
                      // Kadr tayyor bo'lmasa — bo'sh joy: pastda
                      // play belgisi baribir turadi.
                      _VideoThumb(thumbs: thumbs, url: m.mediaUrl),
                      Center(
                        child: Container(
                          width: 52,
                          height: 52,
                          decoration: BoxDecoration(
                            color: Colors.black.withValues(alpha: 0.55),
                            shape: BoxShape.circle,
                            border: Border.all(
                                color: Colors.white.withValues(alpha: 0.3)),
                          ),
                          child: const Icon(Icons.play_arrow_rounded,
                              size: 32, color: Colors.white),
                        ),
                      ),
                    ],
                  ),
                )
              : CachedNetworkImage(
                  cacheManager: AppImageCache.manager,
                  imageUrl: m.mediaUrl,
                  fit: BoxFit.cover,
                  memCacheWidth: 700,
                  placeholder: (_, __) => const SizedBox(
                    height: 150,
                    width: 220,
                    child: Center(
                      child: SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white54),
                      ),
                    ),
                  ),
                  errorWidget: (_, __, ___) => const SizedBox(
                    height: 150,
                    width: 220,
                    child: Icon(Icons.broken_image_outlined,
                        color: Colors.white38),
                  ),
                ),
        ),
      ),
    );
  }
}

/// Matn va vaqt (`ChatMessageCell`): vaqt oxirgi qatorga sig'sa o'sha
/// qatorda o'ngda turadi — matn oxiriga vaqt kengligicha ko'rinmas
/// joy qo'yiladi; sig'masa o'zi yangi qatorga tushadi.
class _TextWithTime extends StatelessWidget {
  final String text;
  final Widget time;
  final double timeWidth;

  const _TextWithTime({
    required this.text,
    required this.time,
    required this.timeWidth,
  });

  @override
  Widget build(BuildContext context) {
    const style = TextStyle(color: Colors.white, fontSize: 16, height: 1.3);
    return Stack(
      children: [
        Text.rich(
          TextSpan(children: [
            // Maxsus emoji (`[ce:..]`) va Telegram emojilari bilan.
            ...(customEmojiSpans(text, style,
                    (p) => emojiSpans(p, style) ?? [TextSpan(text: p)]) ??
                emojiSpans(text, style) ??
                [TextSpan(text: text)]),
            WidgetSpan(child: SizedBox(width: timeWidth, height: 14)),
          ]),
          style: style,
        ),
        Positioned(right: 0, bottom: 1, child: time),
      ],
    );
  }
}

/// VIDEONING BOSHIDAGI KADRI (puffak ichida).
///
/// ── NEGA ALOHIDA VIDJET ─────────────────────────────────────
///
/// Kadr tayyor bo'lganda FAQAT SHU puffak qaytadan chiziladi —
/// butun ro'yxat emas. Uzun yozishmada bu sezilarli farq:
/// aks holda har bir tayyor kadr o'nlab qatorni qayta
/// qurdirardi.
///
/// Hech qanday hisob-kitob bu yerda bo'lmaydi: vidjet faqat
/// XOTIRADAN o'qiydi (`peek`), yasash esa fon'da ketadi.
class _VideoThumb extends StatefulWidget {
  final ChatVideoThumb thumbs;
  final String url;

  const _VideoThumb({required this.thumbs, required this.url});

  @override
  State<_VideoThumb> createState() => _VideoThumbState();
}

class _VideoThumbState extends State<_VideoThumb> {
  @override
  void initState() {
    super.initState();
    widget.thumbs.addListener(_onThumb);
    // Ro'yxat qurilayotgan kadrda tarmoqqa chiqmaymiz: so'rov
    // birinchi kadrdan KEYIN yuboriladi, ya'ni surish silliq
    // qoladi.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.thumbs.request(widget.url);
    });
  }

  @override
  void didUpdateWidget(covariant _VideoThumb old) {
    super.didUpdateWidget(old);
    // Ro'yxat qatorlarni qayta ishlatadi — manzil almashsa
    // yangisi so'raladi.
    if (old.url != widget.url) widget.thumbs.request(widget.url);
  }

  @override
  void dispose() {
    widget.thumbs.removeListener(_onThumb);
    super.dispose();
  }

  void _onThumb() {
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final bytes = widget.thumbs.peek(widget.url);
    if (bytes == null) return const SizedBox.shrink();
    return Image.memory(
      bytes,
      fit: BoxFit.cover,
      // Kadr almashganda puffak bir lahza oqarib ketmasin.
      gaplessPlayback: true,
    );
  }
}

// ══════════════════════════════════════════════════════════════
//  OVOZLI XABAR
// ══════════════════════════════════════════════════════════════
//
// Play/pause tugmasi, surib o'tkaziladigan chiziq va vaqt.
// Ijro YAGONA ijrochida (`VoicePlayer`): boshqa xabar bosilsa
// bunisi o'zi to'xtaydi.

class _VoiceBubble extends StatelessWidget {
  final ChatMessage message;
  final bool mine;
  final VoidCallback? onSelect;

  const _VoiceBubble({
    required this.message,
    required this.mine,
    this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    final m = message;
    final vp = VoicePlayer.instance;
    return AnimatedBuilder(
      animation: vp,
      builder: (context, _) {
        final playing = vp.isPlaying(m.id);
        final opening = vp.isOpening(m.id);
        final pos = vp.positionOf(m.id);
        // Uzunlik: ijro ochilgan bo'lsa fayldan, aks holda
        // xabar bilan kelgan qiymatdan. Ikkovi ham bo'lmasa
        // chiziq bo'sh turadi.
        final real = vp.durationOf(m.id);
        final total = real > Duration.zero
            ? real
            : Duration(milliseconds: m.mediaMs);
        final maxMs = total.inMilliseconds;
        final posMs = pos.inMilliseconds.clamp(0, maxMs <= 0 ? 0 : maxMs);

        // ── TELEGRAM'DAGIDEK (`ChatMessageCell`, `SeekBarWaveform`) ──
        //   * 44 dp tugma: o'z xabarida oq doira + pufak rangidagi belgi,
        //     suhbatdoshnikida urg'u rangi + oq belgi;
        //   * o'ngida to'lqin (30 dp), ostida vaqt (12).
        final wave = tgDecodeWaveform(m.body);
        final current = vp.isCurrent(m.id) && maxMs > 0;
        final progress = maxMs <= 0 ? 0.0 : posMs / maxMs;
        final btnBg = mine ? Colors.white : AppColors.accent;
        final btnFg = mine ? AppColors.accent : Colors.white;
        return SizedBox(
          width: 200,
          child: Row(
            children: [
              GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: onSelect ?? () => vp.toggle(m.id, m.mediaUrl),
                child: Container(
                  width: 44,
                  height: 44,
                  decoration:
                      BoxDecoration(shape: BoxShape.circle, color: btnBg),
                  child: opening
                      ? Padding(
                          padding: const EdgeInsets.all(12),
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: btnFg),
                        )
                      : AnimatedSwitcher(
                          duration: const Duration(milliseconds: 180),
                          transitionBuilder: (c, a) =>
                              ScaleTransition(scale: a, child: c),
                          child: Icon(
                            playing
                                ? Icons.pause_rounded
                                : Icons.play_arrow_rounded,
                            key: ValueKey(playing),
                            size: 28,
                            color: btnFg,
                          ),
                        ),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      height: 30,
                      child: TgWaveform(
                        wave: wave,
                        progress: current ? progress : 0,
                        played: Colors.white,
                        rest: Colors.white.withValues(alpha: 0.4),
                        onSeek: onSelect != null || !current
                            ? null
                            : (f) => vp.seek(m.id,
                                Duration(milliseconds: (f * maxMs).round())),
                      ),
                    ),
                    Text(
                      // Yangrayotganda — hozirgi nuqta, aks holda uzunlik.
                      current ? voiceClock(pos) : voiceClock(total),
                      style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.65),
                        fontSize: 12,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}


// ══════════════════════════════════════════════════════════════
//  YUBORILISH BELGISI
// ══════════════════════════════════════════════════════════════
//
// Uch holat:
//   * yuborilmoqda — AYLANIB turgan soat;
//   * yuborildi    — bitta ✓;
//   * o'qildi      — ikkita ✓✓.

class _SendState extends StatefulWidget {
  final bool pending;
  final bool seen;

  const _SendState({required this.pending, required this.seen});

  @override
  State<_SendState> createState() => _SendStateState();
}

class _SendStateState extends State<_SendState>
    with SingleTickerProviderStateMixin {
  late final AnimationController _spin = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1400),
  );

  @override
  void initState() {
    super.initState();
    if (widget.pending) _spin.repeat();
  }

  @override
  void didUpdateWidget(covariant _SendState old) {
    super.didUpdateWidget(old);
    // Yuborilib bo'lgach aylanish to'xtaydi — bekor aylanayotgan
    // animatsiya batareyani yeydi.
    if (widget.pending && !_spin.isAnimating) {
      _spin.repeat();
    } else if (!widget.pending && _spin.isAnimating) {
      _spin.stop();
    }
  }

  @override
  void dispose() {
    _spin.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = Colors.white.withValues(alpha: widget.seen ? 0.95 : 0.6);
    if (widget.pending) {
      return RotationTransition(
        turns: _spin,
        child: Icon(Icons.schedule_rounded, size: 12, color: color),
      );
    }
    // Ikkita belgi bir-biriga QISMAN kirib turadi — Telegramda
    // ham shunday, alohida ikkita ✓ bo'lib ko'rinmaydi.
    if (widget.seen) {
      return SizedBox(
        width: 17,
        height: 12,
        child: Stack(
          children: [
            Icon(Icons.check_rounded, size: 12, color: color),
            Positioned(
              left: 5,
              child: Icon(Icons.check_rounded, size: 12, color: color),
            ),
          ],
        ),
      );
    }
    return Icon(Icons.check_rounded, size: 12, color: color);
  }
}

// ══════════════════════════════════════════════════════════════
//  YUKLANAYOTGAN FAYL
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): "video, ovozli xabar yoki rasm
// yuborganda to'g'ridan-to'g'ri chat oynasida ko'rinsin va
// progress chizig'i play/pause tugmasi atrofida aylanib
// kattalashsin, huddi Telegramdagidek".
//
// Ya'ni fayl yuborilmasdan TURIB puffak bo'lib paydo bo'ladi:
// rasm bo'lsa o'zi ko'rinadi (telefondagi faylidan, tarmoq
// kutilmaydi), video va ovoz uchun esa tugma atrofida aylana
// to'lib boradi. Pastda aylanuvchi soat — "hali yuborilmadi".

class _UploadingBubble extends StatelessWidget {
  final String type;
  final String path;
  final double progress;
  final int ms;

  const _UploadingBubble({
    required this.type,
    required this.path,
    required this.progress,
    required this.ms,
  });

  @override
  Widget build(BuildContext context) {
    final p = progress.clamp(0.0, 1.0);
    final image = type == 'image';
    // Rasm VA video — ikkovi ham puffakda TASVIR bo'lib turadi
    // (video endi kadri bilan), shu sabab ramka ingichka.
    // Ovozli xabarda esa tasvir yo'q — unga odatdagi ichki
    // masofa qoladi. Yuborilgan xabar puffagi ham shu qoidaga
    // amal qiladi (`isViewable`).
    final wide = image || type == 'video';
    // Dumaloq video — pufaksiz doira, ichida halqa (Telegram'dagidek).
    if (type == 'round') {
      return Padding(
        padding: const EdgeInsets.only(bottom: 8),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            Container(
              width: 210,
              height: 210,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: Colors.black.withValues(alpha: 0.45),
              ),
              alignment: Alignment.center,
              child: _Ring(progress: p),
            ),
          ],
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          Container(
            constraints: BoxConstraints(
              maxWidth: MediaQuery.sizeOf(context).width * 0.76,
            ),
            padding: EdgeInsets.fromLTRB(
                wide ? 4 : 13, wide ? 4 : 9, wide ? 4 : 13, 7),
            decoration: BoxDecoration(
              color: AppColors.accent.withValues(alpha: 0.92),
              borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(16),
                topRight: Radius.circular(16),
                bottomLeft: Radius.circular(16),
                bottomRight: Radius.circular(4),
              ),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              mainAxisSize: MainAxisSize.min,
              children: [
                if (image)
                  ClipRRect(
                    borderRadius: BorderRadius.circular(13),
                    child: Stack(
                      alignment: Alignment.center,
                      children: [
                        ConstrainedBox(
                          constraints: const BoxConstraints(
                              maxHeight: 240, minWidth: 150),
                          child: Image.file(File(path), fit: BoxFit.cover),
                        ),
                        Container(color: Colors.black38),
                        _Ring(progress: p),
                      ],
                    ),
                  )
                else if (type == 'video')
                  ClipRRect(
                    borderRadius: BorderRadius.circular(13),
                    child: SizedBox(
                      height: 150,
                      width: 220,
                      child: Stack(
                        fit: StackFit.expand,
                        children: [
                          DecoratedBox(
                            decoration: BoxDecoration(
                              color: Colors.black.withValues(alpha: 0.35),
                            ),
                          ),
                          Center(child: _Ring(progress: p)),
                        ],
                      ),
                    ),
                  )
                else
                  SizedBox(
                    width: 210,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        _Ring(progress: p, size: 38),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(
                            type == 'file'
                                ? path.split('/').last
                                : voiceClock(Duration(milliseconds: ms)),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: Colors.white.withValues(alpha: 0.85),
                              fontSize: 12,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                // Pastda aylanuvchi soat — hali yuborilmadi.
                const Padding(
                  padding: EdgeInsets.only(top: 4, right: 4),
                  child: _SendState(pending: true, seen: false),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Play tugmasi atrofida aylanib KATTALASHADIGAN progress.
class _Ring extends StatelessWidget {
  final double progress;
  final double size;

  const _Ring({required this.progress, this.size = 52});

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: size,
      height: size,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: Colors.black.withValues(alpha: 0.55),
          shape: BoxShape.circle,
        ),
        child: SpinRing(
          progress: progress,
          size: size,
          color: Colors.white,
          track: Colors.white.withValues(alpha: 0.18),
          fontSize: size > 44 ? 13 : 10,
        ),
      ),
    );
  }
}

/// TELEGRAM'DAGIDEK YUKLASH HALQASI.
///
/// TALAB (foydalanuvchi): "progress chizig'i AYLANGAN holda uzayib
/// birlashsin — bir joyda turgan holda emas; aylana ichidagi foiz
/// admin panelidagidek BIR XIL tezlikda o'ssin, to'xtab-to'xtab emas".
///
///   * Yoy doim aylanadi (bir aylanish ~1.6 s) va shu aylanish
///     davomida uzayadi; 100% da to'liq halqa bo'lib birlashadi.
///   * Yuklash qismlari (512 KB) bo'lak-bo'lak tugaydi, ya'ni haqiqiy
///     qiymat sakrab o'sadi. Ko'rsatiladigan qiymat esa har kadrda
///     o'lchangan TEZLIK bilan bir tekis oshadi va haqiqiy qiymatdan
///     o'zib ketmaydi — foiz raqami to'xtamasdan, bir maromda o'sadi.
class SpinRing extends StatefulWidget {
  final double progress;
  final double size;
  final Color color;
  final Color track;
  final double stroke;
  final double fontSize;

  const SpinRing({
    super.key,
    required this.progress,
    required this.size,
    required this.color,
    required this.track,
    this.stroke = 3,
    this.fontSize = 12,
  });

  @override
  State<SpinRing> createState() => _SpinRingState();
}

class _SpinRingState extends State<SpinRing>
    with SingleTickerProviderStateMixin {
  late final Ticker _ticker;
  Duration _last = Duration.zero;
  double _angle = 0;
  double _shown = 0;

  /// Haqiqiy qiymatning o'sish tezligi (ulush/soniya), silliqlangan.
  double _rate = 0;
  double _prevTarget = 0;
  DateTime _prevAt = DateTime.now();

  @override
  void initState() {
    super.initState();
    _prevTarget = widget.progress.clamp(0.0, 1.0);
    _shown = _prevTarget;
    _ticker = createTicker(_tick)..start();
  }

  @override
  void didUpdateWidget(SpinRing old) {
    super.didUpdateWidget(old);
    final target = widget.progress.clamp(0.0, 1.0);
    if (target < _prevTarget) {
      // Yangi yuklash boshlandi.
      _shown = target;
      _rate = 0;
    } else if (target > _prevTarget) {
      final now = DateTime.now();
      final dt = now.difference(_prevAt).inMicroseconds / 1e6;
      if (dt > 0.05) {
        final r = (target - _prevTarget) / dt;
        _rate = _rate == 0 ? r : _rate * 0.7 + r * 0.3;
        _prevAt = now;
      }
    }
    _prevTarget = target;
  }

  void _tick(Duration now) {
    final dt = _last == Duration.zero
        ? 0.0
        : (now - _last).inMicroseconds / 1e6;
    _last = now;
    final target = widget.progress.clamp(0.0, 1.0);
    // Aylanish: ~1.6 soniyada bir marta.
    _angle = (_angle + dt * 2 * math.pi / 1.6) % (2 * math.pi);
    if (_shown < target) {
      // O'lchangan tezlik bilan bir tekis; ortda qolib ketsa (tezlik
      // hali o'lchanmagan) — farqning bir qismi bilan quvib yetadi.
      final gap = target - _shown;
      final step = math.max(_rate * dt, gap * dt * 1.5);
      _shown = math.min(target, _shown + step);
    }
    setState(() {});
  }

  @override
  void dispose() {
    _ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: widget.size,
      height: widget.size,
      child: CustomPaint(
        painter: _SpinRingPainter(
          angle: _angle,
          value: _shown,
          color: widget.color,
          track: widget.track,
          stroke: widget.stroke,
        ),
        child: Center(
          child: Text(
            '${(_shown * 100).floor()}',
            style: TextStyle(
              color: Colors.white,
              fontSize: widget.fontSize,
              fontWeight: FontWeight.w800,
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
          ),
        ),
      ),
    );
  }
}

class _SpinRingPainter extends CustomPainter {
  final double angle;
  final double value;
  final Color color;
  final Color track;
  final double stroke;

  _SpinRingPainter({
    required this.angle,
    required this.value,
    required this.color,
    required this.track,
    required this.stroke,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final r = (math.min(size.width, size.height) - stroke) / 2;
    final c = size.center(Offset.zero);
    final rect = Rect.fromCircle(center: c, radius: r);
    canvas.drawCircle(
        c,
        r,
        Paint()
          ..color = track
          ..style = PaintingStyle.stroke
          ..strokeWidth = stroke);
    // Eng kichik yoy ham ko'rinsin (0% da ham aylanish sezilsin).
    final sweep = math.max(value, 0.04) * 2 * math.pi;
    final full = value >= 0.999;
    canvas.drawArc(
        rect,
        full ? 0 : angle - math.pi / 2,
        full ? 2 * math.pi : sweep,
        false,
        Paint()
          ..color = color
          ..style = PaintingStyle.stroke
          ..strokeCap = StrokeCap.round
          ..strokeWidth = stroke);
  }

  @override
  bool shouldRepaint(_SpinRingPainter old) =>
      old.angle != angle || old.value != value;
}


// ══════════════════════════════════════════════════════════════
//  DUMALOQ VIDEO XABAR (Telegram'dagidek)
// ══════════════════════════════════════════════════════════════
//
// Ro'yxatda ovozsiz, takrorlanib o'ynaydi. Bosilsa — boshidan, OVOZ
// bilan va atrofida progress halqasi; tugagach yana ovozsiz.
// Fayl chatdagi boshqa videolar kabi shifrlangan diskka keshlanadi
// (`aru://`, `AruDataSource`).

class _RoundBubble extends StatefulWidget {
  final String url;
  final int ms;
  final VoidCallback? onSelect;
  const _RoundBubble({required this.url, required this.ms, this.onSelect});

  @override
  State<_RoundBubble> createState() => _RoundBubbleState();
}

class _RoundBubbleState extends State<_RoundBubble> {
  VideoPlayerController? _c;
  bool _sound = false;
  bool _failed = false;

  /// Nega ochilmadi — pufakchada ko'rinadi (bosilsa qayta uriniladi).
  String _why = '';

  /// Xatodan keyin o'zi bir marta qayta urindimi.
  bool _retried = false;

  /// Telegram'dagidek: ekran qisqa tomonining 60% i
  /// (`roundMessageSize`).
  double get _size =>
      (MediaQuery.sizeOf(context).shortestSide * 0.6).clamp(160.0, 320.0);

  @override
  void initState() {
    super.initState();
    _open();
  }

  Future<void> _open() async {
    final tg = TelegramService.instance;
    final aru = defaultTargetPlatform == TargetPlatform.android;
    final onDisk = aru && RustCore.instance.videoIsComplete(widget.url);
    final local = onDisk ? null : await tg.prepare(widget.url);
    if (!mounted) return;
    if (local != null) tg.hold(this, widget.url);
    if (!onDisk && local == null) {
      setState(() {
        _failed = true;
        _why = 'Video hali tayyor emas';
      });
      return;
    }
    final source = onDisk || aru
        ? TelegramService.aruUri(widget.url)
        : Uri.parse(local!);
    final c = VideoPlayerController.networkUrl(source,
        videoPlayerOptions: VideoPlayerOptions(mixWithOthers: true));
    try {
      await c.initialize();
      await c.setLooping(true);
      await c.setVolume(0);
      await c.play();
    } catch (e) {
      await c.dispose();
      // Dekoder bir zum band bo'lishi mumkin (ro'yxatdagi boshqa
      // video/GIF uni hali bo'shatmagan) — bir marta o'zi qayta
      // uriniladi, keyingisi — bosilganda.
      if (!_retried && mounted) {
        _retried = true;
        await Future<void>.delayed(const Duration(milliseconds: 700));
        if (mounted) return _open();
        return;
      }
      if (mounted) {
        setState(() {
          _failed = true;
          _why = '$e'.split('\n').first;
        });
      }
      return;
    }
    if (!mounted) {
      await c.dispose();
      return;
    }
    c.addListener(_tick);
    setState(() => _c = c);
  }

  void _tick() {
    final c = _c;
    if (c == null) return;
    final v = c.value;
    // Ovozli ijro tugadi — yana ovozsiz, takrorlanib.
    if (_sound && !v.isPlaying && v.position >= v.duration) {
      _sound = false;
      c.setVolume(0);
      c.setLooping(true);
      c.seekTo(Duration.zero);
      c.play();
    }
    if (mounted) setState(() {});
  }

  Future<void> _tap() async {
    final c = _c;
    if (c == null) {
      if (_failed) {
        setState(() {
          _failed = false;
          _why = '';
        });
        await _open();
      }
      return;
    }
    if (!_sound) {
      await VoicePlayer.instance.stop();
      _sound = true;
      await c.setLooping(false);
      await c.seekTo(Duration.zero);
      await c.setVolume(1);
      await c.play();
    } else if (c.value.isPlaying) {
      await c.pause();
    } else {
      await c.play();
    }
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _c?.removeListener(_tick);
    _c?.dispose();
    TelegramService.instance.unhold(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = _c;
    final v = c?.value;
    final total = v != null && v.duration > Duration.zero
        ? v.duration
        : Duration(milliseconds: widget.ms);
    final pos = v?.position ?? Duration.zero;
    final left = total - pos;
    // Telegram: ovoz bilan o'ynaganda doira `roundPlayingMessageSize`
    // gacha kattalashadi (ekran qisqa tomonining ~92% i), tugagach
    // qaytadi.
    final playSize =
        (MediaQuery.sizeOf(context).shortestSide * 0.92 - 16).clamp(200.0, 420.0);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onSelect ?? _tap,
      child: TweenAnimationBuilder<double>(
        tween: Tween(end: _sound ? playSize : _size),
        duration: const Duration(milliseconds: 220),
        curve: Curves.easeOutCubic,
        builder: (context, size, _) => _roundBody(c, v, size, total, pos, left),
      ),
    );
  }

  Widget _roundBody(VideoPlayerController? c, VideoPlayerValue? v,
      double size, Duration total, Duration pos, Duration left) {
    final _size = size;
    return SizedBox(
        width: _size,
        height: _size,
        child: Stack(
          alignment: Alignment.center,
          children: [
            ClipOval(
              child: Container(
                width: _size - 8,
                height: _size - 8,
                color: Colors.white.withValues(alpha: 0.06),
                child: c == null
                    ? Center(
                        child: _failed
                            ? Column(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Icon(Icons.refresh_rounded,
                                      size: 30,
                                      color:
                                          Colors.white.withValues(alpha: 0.6)),
                                  const SizedBox(height: 6),
                                  Padding(
                                    padding: const EdgeInsets.symmetric(
                                        horizontal: 24),
                                    child: Text(_why,
                                        textAlign: TextAlign.center,
                                        maxLines: 3,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                            fontSize: 11,
                                            color: Colors.white
                                                .withValues(alpha: 0.5))),
                                  ),
                                ],
                              )
                            : const SizedBox(
                                width: 24,
                                height: 24,
                                child: CircularProgressIndicator(
                                    strokeWidth: 2, color: Colors.white54),
                              ),
                      )
                    : FittedBox(
                        fit: BoxFit.cover,
                        child: SizedBox(
                          width: v!.size.width,
                          height: v.size.height,
                          child: VideoPlayer(c),
                        ),
                      ),
              ),
            ),
            // Ovoz bilan o'ynayotganda — atrofida progress.
            if (_sound && total > Duration.zero)
              SizedBox(
                width: _size,
                height: _size,
                child: CircularProgressIndicator(
                  value: (pos.inMilliseconds / total.inMilliseconds)
                      .clamp(0.0, 1.0),
                  strokeWidth: 3,
                  color: Colors.white,
                  backgroundColor: Colors.transparent,
                ),
              ),
            // Qolgan vaqt va "ovozsiz" belgisi.
            Positioned(
              left: 18,
              bottom: 14,
              child: Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
                decoration: BoxDecoration(
                  color: Colors.black.withValues(alpha: 0.45),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      voiceClock(_sound ? left : total),
                      style: const TextStyle(
                          color: Colors.white, fontSize: 11.5),
                    ),
                    if (!_sound) ...[
                      const SizedBox(width: 4),
                      const Icon(Icons.volume_off_rounded,
                          size: 13, color: Colors.white),
                    ],
                  ],
                ),
              ),
            ),
          ],
        ),
      );
  }
}


// ══════════════════════════════════════════════════════════════
//  FAYL XABARI (Telegram'dagidek: belgi, nom; bosilsa ochiladi)
// ══════════════════════════════════════════════════════════════

class _FileBubble extends StatefulWidget {
  final String url;
  final String name;
  final bool mine;
  final VoidCallback? onSelect;
  const _FileBubble(
      {required this.url, required this.name, required this.mine, this.onSelect});

  @override
  State<_FileBubble> createState() => _FileBubbleState();
}

class _FileBubbleState extends State<_FileBubble> {
  bool _busy = false;

  Future<void> _open() async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final name = TelegramService.fileNameOf(widget.url);
      final bytes = await TelegramService.instance.fetchBytes(widget.url);
      if (bytes == null) throw 'yuklab bo\'lmadi';
      final dir = await getTemporaryDirectory();
      final safe = widget.name.replaceAll(RegExp(r'[\\/:*?"<>|]'), '_');
      final f = File('${dir.path}/files/${name.hashCode}_${safe.isEmpty ? name : safe}');
      await f.parent.create(recursive: true);
      await f.writeAsBytes(bytes, flush: true);
      await OpenFilex.open(f.path);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.maybeOf(context)
            ?.showSnackBar(SnackBar(content: Text('Fayl ochilmadi: $e')));
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final dot = widget.name.lastIndexOf('.');
    final ext = dot > 0 ? widget.name.substring(dot + 1).toUpperCase() : '';
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onSelect ?? _open,
      child: SizedBox(
        width: 230,
        child: Row(
          children: [
            Container(
              width: 46,
              height: 46,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: widget.mine
                    ? Colors.white.withValues(alpha: 0.22)
                    : AppColors.accent,
              ),
              child: _busy
                  ? const Padding(
                      padding: EdgeInsets.all(13),
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white),
                    )
                  : const Icon(Icons.insert_drive_file_rounded,
                      color: Colors.white),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    widget.name.isEmpty ? 'Fayl' : widget.name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        color: Colors.white,
                        fontSize: 14,
                        fontWeight: FontWeight.w600),
                  ),
                  if (ext.isNotEmpty)
                    Text(ext,
                        style: TextStyle(
                            color: Colors.white.withValues(alpha: 0.6),
                            fontSize: 12)),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
