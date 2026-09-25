// lib/widgets/tg_countries.dart — kirish oynasi uchun davlatlar
// ro'yxati (Telegram'dagidek: bayroq, nom, kod va raqam shabloni).
//
// Shablonda `X` — bitta raqam, bo'sh joy — ajratgich. Shablon
// ko'rsatilmagan davlatda raqam bo'laklarga bo'linmaydi.

class TgCountry {
  final String iso;
  final String name;
  final String code;
  final String pattern;
  const TgCountry(this.iso, this.name, this.code, [this.pattern = '']);

  /// ISO kodidan bayroq emojisi (`UZ` -> 🇺🇿).
  String get flag => String.fromCharCodes(
      iso.toUpperCase().codeUnits.map((c) => 0x1F1E6 + c - 65));

  /// Shablondagi raqamlar soni (0 — cheklanmagan).
  int get length => pattern.replaceAll(' ', '').length;

  /// Raqamni shablon bo'yicha bo'laklaydi: `901234567` -> `90 123 45 67`.
  String format(String digits) {
    if (pattern.isEmpty) return digits;
    final b = StringBuffer();
    var i = 0;
    for (final ch in pattern.split('')) {
      if (i >= digits.length) break;
      if (ch == ' ') {
        b.write(' ');
      } else {
        b.write(digits[i++]);
      }
    }
    if (i < digits.length) b.write(digits.substring(i));
    return b.toString();
  }
}

/// Telegram'dagi barcha davlatlar (Telegram Android `countries.txt`),
/// nom bo'yicha saralangan. Asosiy davlatlar nomi o'zbekcha.
const tgCountries = <TgCountry>[
  TgCountry('AF', 'Afg\'oniston', '93', 'XXX XXX XXX'),
  TgCountry('AL', 'Albaniya', '355', 'XX XXX XXXX'),
  TgCountry('AS', 'American Samoa', '1684', 'XXX XXXX'),
  TgCountry('AD', 'Andorra', '376', 'XX XX XX'),
  TgCountry('AO', 'Angola', '244', 'XXX XXX XXX'),
  TgCountry('AI', 'Anguilla', '1264', 'XXX XXXX'),
  TgCountry('FT', 'Anonymous Numbers', '888', 'XXXX XXXX'),
  TgCountry('AG', 'Antigua & Barbuda', '1268', 'XXX XXXX'),
  TgCountry('US', 'AQSH', '1', 'XXX XXX XXXX'),
  TgCountry('AR', 'Argentina', '54'),
  TgCountry('AM', 'Armaniston', '374', 'XX XXX XXX'),
  TgCountry('AW', 'Aruba', '297', 'XXX XXXX'),
  TgCountry('AU', 'Avstraliya', '61', 'XXX XXX XXX'),
  TgCountry('AT', 'Avstriya', '43'),
  TgCountry('AE', 'BAA', '971', 'XX XXX XXXX'),
  TgCountry('BS', 'Bahamas', '1242', 'XXX XXXX'),
  TgCountry('BH', 'Bahrayn', '973', 'XXXX XXXX'),
  TgCountry('BD', 'Bangladesh', '880'),
  TgCountry('BB', 'Barbados', '1246', 'XXX XXXX'),
  TgCountry('BY', 'Belarus', '375', 'XX XXX XXXX'),
  TgCountry('BE', 'Belgiya', '32', 'XXX XX XX XX'),
  TgCountry('BZ', 'Belize', '501'),
  TgCountry('BJ', 'Benin', '229', 'XX XXX XXX'),
  TgCountry('BM', 'Bermuda', '1441', 'XXX XXXX'),
  TgCountry('BT', 'Bhutan', '975', 'XX XXX XXX'),
  TgCountry('BG', 'Bolgariya', '359'),
  TgCountry('BO', 'Bolivia', '591', 'X XXX XXXX'),
  TgCountry('BQ', 'Bonaire, Sint Eustatius & Saba', '599'),
  TgCountry('BA', 'Bosnia & Herzegovina', '387', 'XX XXX XXX'),
  TgCountry('BW', 'Botswana', '267', 'XX XXX XXX'),
  TgCountry('BR', 'Braziliya', '55', 'XX XXXXX XXXX'),
  TgCountry('VG', 'British Virgin Islands', '1284', 'XXX XXXX'),
  TgCountry('BN', 'Brunei Darussalam', '673', 'XXX XXXX'),
  TgCountry('BF', 'Burkina Faso', '226', 'XX XX XX XX'),
  TgCountry('BI', 'Burundi', '257', 'XX XX XXXX'),
  TgCountry('GB', 'Buyuk Britaniya', '44', 'XXXX XXXXXX'),
  TgCountry('KH', 'Cambodia', '855'),
  TgCountry('CM', 'Cameroon', '237', 'XXXX XXXX'),
  TgCountry('CV', 'Cape Verde', '238', 'XXX XXXX'),
  TgCountry('KY', 'Cayman Islands', '1345', 'XXX XXXX'),
  TgCountry('CF', 'Central African Rep.', '236', 'XX XX XX XX'),
  TgCountry('TD', 'Chad', '235', 'XX XX XX XX'),
  TgCountry('CZ', 'Chexiya', '420', 'XXX XXX XXX'),
  TgCountry('CL', 'Chile', '56', 'X XXXX XXXX'),
  TgCountry('CO', 'Colombia', '57', 'XXX XXX XXXX'),
  TgCountry('KM', 'Comoros', '269', 'XXX XXXX'),
  TgCountry('CD', 'Congo (Dem. Rep.)', '243', 'XX XXX XXXX'),
  TgCountry('CG', 'Congo (Rep.)', '242', 'XX XXX XXXX'),
  TgCountry('CK', 'Cook Islands', '682'),
  TgCountry('CR', 'Costa Rica', '506', 'XXXX XXXX'),
  TgCountry('CW', 'Curaçao', '599'),
  TgCountry('CI', 'Côte d`Ivoire', '225', 'XX XXX XXX'),
  TgCountry('DK', 'Daniya', '45', 'XXXX XXXX'),
  TgCountry('IO', 'Diego Garcia', '246', 'XXX XXXX'),
  TgCountry('DJ', 'Djibouti', '253', 'XX XX XX XX'),
  TgCountry('DM', 'Dominica', '1767', 'XXX XXXX'),
  TgCountry('DO', 'Dominican Rep.', '1', 'XXX XXX XXXX'),
  TgCountry('EC', 'Ecuador', '593', 'XX XXX XXXX'),
  TgCountry('ET', 'Efiopiya', '251', 'XX XXX XXXX'),
  TgCountry('SV', 'El Salvador', '503', 'XXXX XXXX'),
  TgCountry('GQ', 'Equatorial Guinea', '240', 'XXX XXX XXX'),
  TgCountry('ER', 'Eritrea', '291', 'X XXX XXX'),
  TgCountry('IR', 'Eron', '98', 'XXX XXX XXXX'),
  TgCountry('EE', 'Estoniya', '372'),
  TgCountry('FK', 'Falkland Islands', '500'),
  TgCountry('FO', 'Faroe Islands', '298', 'XXX XXX'),
  TgCountry('FJ', 'Fiji', '679'),
  TgCountry('PH', 'Filippin', '63', 'XXX XXX XXXX'),
  TgCountry('FI', 'Finlandiya', '358'),
  TgCountry('FR', 'Fransiya', '33', 'X XX XX XX XX'),
  TgCountry('GF', 'French Guiana', '594'),
  TgCountry('PF', 'French Polynesia', '689'),
  TgCountry('GA', 'Gabon', '241', 'X XX XX XX'),
  TgCountry('GM', 'Gambia', '220', 'XXX XXXX'),
  TgCountry('DE', 'Germaniya', '49'),
  TgCountry('GH', 'Ghana', '233'),
  TgCountry('GI', 'Gibraltar', '350', 'XXXX XXXX'),
  TgCountry('HK', 'Gonkong', '852', 'X XXX XXXX'),
  TgCountry('GL', 'Greenland', '299', 'XXX XXX'),
  TgCountry('GD', 'Grenada', '1473', 'XXX XXXX'),
  TgCountry('GR', 'Gretsiya', '30', 'XXX XXX XXXX'),
  TgCountry('GE', 'Gruziya', '995', 'XXX XXX XXX'),
  TgCountry('GP', 'Guadeloupe', '590', 'XXX XX XX XX'),
  TgCountry('GU', 'Guam', '1671', 'XXX XXXX'),
  TgCountry('GT', 'Guatemala', '502', 'X XXX XXXX'),
  TgCountry('GN', 'Guinea', '224', 'XXX XXX XXX'),
  TgCountry('GW', 'Guinea-Bissau', '245', 'XXX XXXX'),
  TgCountry('GY', 'Guyana', '592'),
  TgCountry('HT', 'Haiti', '509'),
  TgCountry('IN', 'Hindiston', '91', 'XXXXX XXXXX'),
  TgCountry('HN', 'Honduras', '504', 'XXXX XXXX'),
  TgCountry('ID', 'Indoneziya', '62'),
  TgCountry('JO', 'Iordaniya', '962', 'X XXXX XXXX'),
  TgCountry('IE', 'Irlandiya', '353', 'XX XXX XXXX'),
  TgCountry('IQ', 'Iroq', '964', 'XXX XXX XXXX'),
  TgCountry('IS', 'Islandiya', '354', 'XXX XXXX'),
  TgCountry('ES', 'Ispaniya', '34', 'XXX XXX XXX'),
  TgCountry('IL', 'Isroil', '972', 'XX XXX XXXX'),
  TgCountry('IT', 'Italiya', '39'),
  TgCountry('JM', 'Jamaica', '1876', 'XXX XXXX'),
  TgCountry('ZA', 'Janubiy Afrika', '27', 'XX XXX XXXX'),
  TgCountry('KR', 'Janubiy Koreya', '82'),
  TgCountry('DZ', 'Jazoir', '213', 'XXX XX XX XX'),
  TgCountry('CA', 'Kanada', '1', 'XXX XXX XXXX'),
  TgCountry('KE', 'Keniya', '254', 'XXX XXX XXX'),
  TgCountry('CY', 'Kipr', '357', 'XXXX XXXX'),
  TgCountry('KI', 'Kiribati', '686'),
  TgCountry('XK', 'Kosovo', '383', 'XXXX XXXX'),
  TgCountry('CU', 'Kuba', '53', 'XXXX XXXX'),
  TgCountry('LA', 'Laos', '856', 'XX XX XXX XXX'),
  TgCountry('LV', 'Latviya', '371', 'XXX XXXXX'),
  TgCountry('LS', 'Lesotho', '266', 'XX XXX XXX'),
  TgCountry('LR', 'Liberia', '231'),
  TgCountry('LI', 'Liechtenstein', '423'),
  TgCountry('LT', 'Litva', '370', 'XXX XXXXX'),
  TgCountry('LB', 'Livan', '961'),
  TgCountry('LY', 'Liviya', '218', 'XX XXX XXXX'),
  TgCountry('LU', 'Luxembourg', '352'),
  TgCountry('MO', 'Macau', '853', 'XXXX XXXX'),
  TgCountry('MK', 'Macedonia', '389', 'XX XXX XXX'),
  TgCountry('MG', 'Madagascar', '261', 'XX XX XXX XX'),
  TgCountry('MW', 'Malawi', '265', '77 XXX XXXX'),
  TgCountry('MY', 'Malayziya', '60'),
  TgCountry('MV', 'Maldives', '960', 'XXX XXXX'),
  TgCountry('ML', 'Mali', '223', 'XXXX XXXX'),
  TgCountry('MT', 'Malta', '356', 'XX XX XX XX'),
  TgCountry('MA', 'Marokash', '212', 'XX XXX XXXX'),
  TgCountry('MH', 'Marshall Islands', '692'),
  TgCountry('MQ', 'Martinique', '596'),
  TgCountry('MR', 'Mauritania', '222', 'XXXX XXXX'),
  TgCountry('MU', 'Mauritius', '230'),
  TgCountry('MX', 'Meksika', '52'),
  TgCountry('FM', 'Micronesia', '691'),
  TgCountry('EG', 'Misr', '20', 'XX XXXX XXXX'),
  TgCountry('MN', 'Mo\'g\'uliston', '976', 'XX XX XXXX'),
  TgCountry('MD', 'Moldova', '373', 'XX XXX XXX'),
  TgCountry('MC', 'Monaco', '377', 'XXXX XXXX'),
  TgCountry('ME', 'Montenegro', '382'),
  TgCountry('MS', 'Montserrat', '1664', 'XXX XXXX'),
  TgCountry('MZ', 'Mozambique', '258', 'XX XXX XXXX'),
  TgCountry('MM', 'Myanma', '95'),
  TgCountry('NA', 'Namibia', '264', 'XX XXX XXXX'),
  TgCountry('NR', 'Nauru', '674'),
  TgCountry('NP', 'Nepal', '977', 'XX XXXX XXXX'),
  TgCountry('NC', 'New Caledonia', '687'),
  TgCountry('NI', 'Nicaragua', '505', 'XXXX XXXX'),
  TgCountry('NL', 'Niderlandiya', '31', 'X XX XX XX XX'),
  TgCountry('NE', 'Niger', '227', 'XX XX XX XX'),
  TgCountry('NG', 'Nigeriya', '234'),
  TgCountry('NU', 'Niue', '683'),
  TgCountry('NF', 'Norfolk Island', '672'),
  TgCountry('MP', 'Northern Mariana Islands', '1670', 'XXX XXXX'),
  TgCountry('NO', 'Norvegiya', '47', 'XXXX XXXX'),
  TgCountry('AZ', 'Ozarbayjon', '994', 'XX XXX XXXX'),
  TgCountry('UZ', 'O\'zbekiston', '998', 'XX XXX XX XX'),
  TgCountry('PW', 'Palau', '680'),
  TgCountry('PS', 'Palestine', '970', 'XXX XX XXXX'),
  TgCountry('PA', 'Panama', '507', 'XXXX XXXX'),
  TgCountry('PG', 'Papua New Guinea', '675'),
  TgCountry('PY', 'Paraguay', '595', 'XXX XXX XXX'),
  TgCountry('PE', 'Peru', '51', 'XXX XXX XXX'),
  TgCountry('PK', 'Pokiston', '92', 'XXX XXX XXXX'),
  TgCountry('PL', 'Polsha', '48', 'XXX XXX XXX'),
  TgCountry('PT', 'Portugaliya', '351', 'X XXXX XXXX'),
  TgCountry('PR', 'Puerto Rico', '1', 'XXX XXX XXXX'),
  TgCountry('QA', 'Qatar', '974', 'XX XXX XXX'),
  TgCountry('KG', 'Qirg\'iziston', '996', 'XXX XXXXXX'),
  TgCountry('KZ', 'Qozog\'iston', '7', 'XXX XXX XX XX'),
  TgCountry('KW', 'Quvayt', '965', 'XXXX XXXX'),
  TgCountry('RU', 'Rossiya', '7', 'XXX XXX XXXX'),
  TgCountry('RO', 'Ruminiya', '40', 'XXX XXX XXX'),
  TgCountry('RW', 'Rwanda', '250', 'XXX XXX XXX'),
  TgCountry('RE', 'Réunion', '262', 'XXX XXX XXX'),
  TgCountry('SH', 'Saint Helena', '290', 'XX XXX'),
  TgCountry('SH', 'Saint Helena', '247', 'XXXX'),
  TgCountry('KN', 'Saint Kitts & Nevis', '1869', 'XXX XXXX'),
  TgCountry('LC', 'Saint Lucia', '1758', 'XXX XXXX'),
  TgCountry('PM', 'Saint Pierre & Miquelon', '508'),
  TgCountry('VC', 'Saint Vincent & the Grenadines', '1784', 'XXX XXXX'),
  TgCountry('WS', 'Samoa', '685'),
  TgCountry('SM', 'San Marino', '378', 'XXX XXX XXXX'),
  TgCountry('SA', 'Saudiya Arabistoni', '966', 'XX XXX XXXX'),
  TgCountry('SN', 'Senegal', '221', 'XX XXX XXXX'),
  TgCountry('RS', 'Serbiya', '381', 'XX XXX XXXX'),
  TgCountry('SC', 'Seychelles', '248', 'X XX XX XX'),
  TgCountry('KP', 'Shimoliy Koreya', '850'),
  TgCountry('LK', 'Shri-Lanka', '94', 'XX XXX XXXX'),
  TgCountry('SE', 'Shvetsiya', '46', 'XX XXX XXXX'),
  TgCountry('CH', 'Shveytsariya', '41', 'XX XXX XXXX'),
  TgCountry('SL', 'Sierra Leone', '232', 'XX XXX XXX'),
  TgCountry('SG', 'Singapur', '65', 'XXXX XXXX'),
  TgCountry('SX', 'Sint Maarten', '1721', 'XXX XXXX'),
  TgCountry('SK', 'Slovakiya', '421', 'XXX XXX XXX'),
  TgCountry('SI', 'Sloveniya', '386', 'XX XXX XXX'),
  TgCountry('SB', 'Solomon Islands', '677'),
  TgCountry('SO', 'Somalia', '252', 'XX XXX XXX'),
  TgCountry('SS', 'South Sudan', '211', 'XX XXX XXXX'),
  TgCountry('SD', 'Sudan', '249', 'XX XXX XXXX'),
  TgCountry('SR', 'Suriname', '597', 'XXX XXXX'),
  TgCountry('SY', 'Suriya', '963', 'XXX XXX XXX'),
  TgCountry('SZ', 'Swaziland', '268', 'XXXX XXXX'),
  TgCountry('ST', 'São Tomé & Príncipe', '239', 'XX XXXXX'),
  TgCountry('TH', 'Tailand', '66', 'X XXXX XXXX'),
  TgCountry('TZ', 'Tanzania', '255', 'XX XXX XXXX'),
  TgCountry('TW', 'Tayvan', '886', 'XXX XXX XXX'),
  TgCountry('TL', 'Timor-Leste', '670'),
  TgCountry('TG', 'Togo', '228', 'XX XXX XXX'),
  TgCountry('TJ', 'Tojikiston', '992', 'XX XXX XXXX'),
  TgCountry('TK', 'Tokelau', '690'),
  TgCountry('TO', 'Tonga', '676'),
  TgCountry('TT', 'Trinidad & Tobago', '1868', 'XXX XXXX'),
  TgCountry('TN', 'Tunis', '216', 'XX XXX XXX'),
  TgCountry('TR', 'Turkiya', '90', 'XXX XXX XXXX'),
  TgCountry('TM', 'Turkmaniston', '993', 'XX XXXXXX'),
  TgCountry('TC', 'Turks & Caicos Islands', '1649', 'XXX XXXX'),
  TgCountry('TV', 'Tuvalu', '688'),
  TgCountry('UG', 'Uganda', '256', 'XX XXX XXXX'),
  TgCountry('UA', 'Ukraina', '380', 'XX XXX XX XX'),
  TgCountry('OM', 'Ummon', '968', 'XXXX XXXX'),
  TgCountry('UY', 'Uruguay', '598', 'X XXX XXXX'),
  TgCountry('VI', 'US Virgin Islands', '1340', 'XXX XXXX'),
  TgCountry('VU', 'Vanuatu', '678'),
  TgCountry('VE', 'Venezuela', '58', 'XXX XXX XXXX'),
  TgCountry('HU', 'Vengriya', '36', 'XXX XXX XXX'),
  TgCountry('VN', 'Vyetnam', '84'),
  TgCountry('WF', 'Wallis & Futuna', '681'),
  TgCountry('CN', 'Xitoy', '86', 'XXX XXXX XXXX'),
  TgCountry('HR', 'Xorvatiya', '385'),
  TgCountry('YL', 'Y-land', '42'),
  TgCountry('YE', 'Yaman', '967', 'XXX XXX XXX'),
  TgCountry('NZ', 'Yangi Zelandiya', '64'),
  TgCountry('JP', 'Yaponiya', '81', 'XX XXXX XXXX'),
  TgCountry('ZM', 'Zambia', '260', 'XX XXX XXXX'),
  TgCountry('ZW', 'Zimbabwe', '263', 'XX XXX XXXX'),
];

/// Bir kodni bir nechta davlat bo'lishsa — qaysi biri tanlanadi.
const _mainOfCode = {'1': 'US', '7': 'RU', '44': 'GB', '39': 'IT', '61': 'AU', '262': 'RE', '358': 'FI', '590': 'GP', '599': 'CW', '47': 'NO', '212': 'MA'};

/// ISO kodi bo'yicha davlat (`UZ`).
TgCountry? countryByIso(String iso) {
  final up = iso.toUpperCase();
  for (final c in tgCountries) {
    if (c.iso == up) return c;
  }
  return null;
}

/// Kod bo'yicha davlat. Bir kodda bir nechta davlat bo'lsa
/// (masalan `7`) [prefer] saqlanadi.
TgCountry? countryByCode(String code, {TgCountry? prefer}) {
  if (code.isEmpty) return null;
  if (prefer != null && prefer.code == code) return prefer;
  final main = _mainOfCode[code];
  final byMain = main == null ? null : countryByIso(main);
  if (byMain != null) return byMain;
  for (final c in tgCountries) {
    if (c.code == code) return c;
  }
  return null;
}

/// To'liq raqamdan (`998901234567`) davlat va qolgan qism —
/// eng uzun mos kod olinadi.
(TgCountry?, String) splitPhone(String digits) {
  for (var n = 4; n >= 1; n--) {
    if (digits.length < n) continue;
    final c = countryByCode(digits.substring(0, n));
    if (c != null) return (c, digits.substring(n));
  }
  return (null, digits);
}
