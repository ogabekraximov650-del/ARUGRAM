package uz.arumediatv.com.ui.profile

// AVTOMATIK YASALGAN: lib/widgets/utyan_parts.dart dan (Telegram `utyan` jo'jasining vektor shakllari, 512 x 512 fazo).
// `a` — oldga qaragan holat, `b` — o'ngga burilgan. Yo'l kodi: 0 moveTo(x,y), 1 cubicTo(6 son), 2 lineTo(x,y), 3 close.

internal class UtyanPart(
    val name: String,
    val fill: Int?,
    val stroke: Int?,
    val width: Float,
    val butt: Boolean,
    val a: FloatArray,
    val b: FloatArray,
)

internal val kUtyanParts: List<UtyanPart> = listOf(
    UtyanPart("body", 0xFFFFD527.toInt(), 0xFFFA9016.toInt(), 10.00f, false, floatArrayOf(0.0f, 281.38f, 392.23f, 1.0f, 229.0f, 392.23f, 141.03f, 376.01f, 147.76f, 312.64f, 1.0f, 148.89f, 302.08f, 147.39f, 292.01f, 162.01f, 258.44f, 1.0f, 190.07f, 194.01f, 228.98f, 181.04f, 297.93f, 181.04f, 1.0f, 357.78f, 181.04f, 409.49f, 215.49f, 425.25f, 271.16f, 1.0f, 429.3f, 285.48f, 447.48f, 302.2f, 446.31f, 319.1f, 1.0f, 442.14f, 379.1f, 342.29f, 392.23f, 281.38f, 392.23f, 3.0f),
        floatArrayOf(0.0f, 283.69f, 390.23f, 1.0f, 230.78f, 390.23f, 144.94f, 374.18f, 155.19f, 307.95f, 1.0f, 156.8f, 297.55f, 166.94f, 292.96f, 181.71f, 259.72f, 1.0f, 210.06f, 195.9f, 239.35f, 183.06f, 309.0f, 183.06f, 1.0f, 368.57f, 183.06f, 420.15f, 216.19f, 436.87f, 269.9f, 1.0f, 441.49f, 284.72f, 443.45f, 301.11f, 442.19f, 318.8f, 1.0f, 437.98f, 378.22f, 345.22f, 390.23f, 283.69f, 390.23f, 3.0f)),
    UtyanPart("head_bl3", null, 0xFFFCEE21.toInt(), 8.00f, false, floatArrayOf(0.0f, 186.34f, 359.0f, 1.0f, 171.75f, 351.75f, 165.87f, 339.95f, 165.14f, 336.75f),
        floatArrayOf(0.0f, 196.39f, 359.35f, 1.0f, 181.76f, 352.27f, 175.76f, 340.73f, 174.9f, 337.38f)),
    UtyanPart("head", 0xFFFFD527.toInt(), 0xFFFA9016.toInt(), 10.00f, false, floatArrayOf(0.0f, 412.07f, 254.45f, 1.0f, 438.06f, 232.17f, 453.15f, 201.03f, 453.15f, 164.5f, 1.0f, 453.15f, 86.89f, 385.47f, 10.5f, 301.98f, 10.5f, 1.0f, 218.49f, 10.5f, 150.81f, 86.89f, 150.81f, 164.5f, 1.0f, 150.81f, 201.12f, 165.98f, 232.13f, 191.64f, 254.23f),
        floatArrayOf(0.0f, 424.28f, 255.76f, 1.0f, 450.53f, 233.7f, 465.78f, 202.86f, 465.78f, 166.68f, 1.0f, 465.78f, 89.83f, 397.42f, 14.17f, 313.09f, 14.17f, 1.0f, 228.76f, 14.17f, 160.4f, 89.83f, 160.4f, 166.68f, 1.0f, 160.4f, 202.95f, 175.72f, 233.66f, 201.64f, 255.55f)),
    UtyanPart("head_bl1", null, 0xFFFFFFFF.toInt(), 8.00f, false, floatArrayOf(0.0f, 189.99f, 112.25f, 1.0f, 197.04f, 97.21f, 221.38f, 54.19f, 275.11f, 39.75f),
        floatArrayOf(0.0f, 200.17f, 114.53f, 1.0f, 207.56f, 99.24f, 232.29f, 57.09f, 286.38f, 43.03f)),
    UtyanPart("head_bl2", null, 0xFFFCEE21.toInt(), 8.00f, false, floatArrayOf(0.0f, 390.1f, 75.13f, 1.0f, 414.1f, 97.74f, 429.55f, 128.67f, 430.97f, 160.22f),
        floatArrayOf(0.0f, 404.79f, 80.74f, 1.0f, 428.33f, 103.7f, 443.07f, 134.58f, 443.47f, 165.82f)),
    UtyanPart("beak", 0xFFFF5D1F.toInt(), 0xFFCF3502.toInt(), 8.00f, true, floatArrayOf(0.0f, 343.86f, 192.51f, 1.0f, 330.59f, 177.99f, 339.6f, 156.99f, 326.3f, 158.44f, 1.0f, 313.7f, 159.8f, 318.27f, 179.23f, 302.93f, 193.0f, 1.0f, 288.14f, 206.23f, 296.23f, 234.33f, 324.53f, 233.7f, 1.0f, 346.3f, 233.2f, 357.85f, 205.6f, 343.86f, 192.51f, 3.0f),
        floatArrayOf(0.0f, 425.83f, 185.6f, 1.0f, 410.38f, 173.14f, 415.79f, 151.96f, 402.28f, 152.07f, 1.0f, 389.49f, 152.18f, 401.86f, 171.28f, 385.06f, 183.21f, 1.0f, 362.4f, 199.28f, 367.79f, 225.51f, 401.89f, 229.77f, 1.0f, 427.1f, 232.91f, 450.58f, 205.27f, 425.83f, 185.6f, 3.0f)),
    UtyanPart("beak_bl", null, 0xFFFFAF7B.toInt(), 8.00f, false, floatArrayOf(0.0f, 313.26f, 196.65f, 1.0f, 321.51f, 190.71f, 326.31f, 180.85f, 326.28f, 172.67f),
        floatArrayOf(0.0f, 390.77f, 191.3f, 1.0f, 399.54f, 186.3f, 403.96f, 177.12f, 404.72f, 169.05f)),
    UtyanPart("mouth", 0xFF7D0909.toInt(), 0xFF7D0909.toInt(), 9.02f, false, floatArrayOf(0.0f, 328.88f, 210.05f, 1.0f, 329.49f, 213.14f, 326.7f, 216.33f, 322.66f, 217.18f, 1.0f, 318.62f, 218.04f, 314.86f, 216.23f, 314.26f, 213.14f, 1.0f, 313.65f, 210.05f, 316.44f, 206.86f, 320.48f, 206.01f, 1.0f, 324.52f, 205.15f, 328.28f, 206.96f, 328.88f, 210.05f, 3.0f),
        floatArrayOf(0.0f, 409.07f, 202.57f, 1.0f, 409.47f, 206.38f, 405.66f, 209.92f, 400.57f, 210.46f, 1.0f, 395.49f, 211.0f, 391.04f, 208.35f, 390.65f, 204.54f, 1.0f, 390.26f, 200.73f, 394.06f, 197.2f, 399.15f, 196.66f, 1.0f, 404.24f, 196.11f, 408.68f, 198.76f, 409.07f, 202.57f, 3.0f)),
    UtyanPart("eye", null, 0xFF000000.toInt(), 12.00f, false, floatArrayOf(0.0f, 280.36f, 161.48f, 1.0f, 265.65f, 186.87f, 233.65f, 174.84f, 230.23f, 157.23f),
        floatArrayOf(0.0f, 356.8f, 153.34f, 1.0f, 339.65f, 176.92f, 308.6f, 161.9f, 306.79f, 144.21f)),
    UtyanPart("eye_2", null, 0xFF000000.toInt(), 12.00f, false, floatArrayOf(0.0f, 424.56f, 170.54f, 1.0f, 415.06f, 191.82f, 379.68f, 190.79f, 375.43f, 165.66f),
        floatArrayOf(0.0f, 471.35f, 170.53f, 1.0f, 465.54f, 190.53f, 439.95f, 186.64f, 435.64f, 161.78f)),
    UtyanPart("hand1", 0xFFFFD527.toInt(), 0xFFFA9016.toInt(), 10.00f, false, floatArrayOf(0.0f, 319.18f, 245.57f, 1.0f, 280.84f, 232.57f, 237.48f, 229.96f, 228.18f, 235.82f, 1.0f, 210.47f, 246.99f, 269.48f, 291.64f, 321.2f, 316.31f),
        floatArrayOf(0.0f, 419.05f, 247.73f, 1.0f, 380.08f, 225.61f, 355.44f, 232.13f, 352.67f, 242.52f, 1.0f, 349.54f, 252.57f, 402.13f, 286.22f, 420.99f, 311.75f)),
    UtyanPart("hand_bl", null, 0xFFFFFFFF.toInt(), 8.05f, false, floatArrayOf(0.0f, 290.07f, 251.6f, 1.0f, 279.38f, 248.0f, 262.13f, 245.62f, 248.84f, 245.83f),
        floatArrayOf(0.0f, 405.83f, 252.54f, 1.0f, 399.16f, 248.7f, 383.84f, 242.23f, 371.53f, 244.01f)),
    UtyanPart("hand2", 0xFFFFD527.toInt(), 0xFFFA9016.toInt(), 10.00f, false, floatArrayOf(0.0f, 205.81f, 264.2f, 1.0f, 233.84f, 268.16f, 264.29f, 258.41f, 273.94f, 271.92f, 1.0f, 283.34f, 284.56f, 242.1f, 317.36f, 199.21f, 327.33f),
        floatArrayOf(0.0f, 300.19f, 259.77f, 1.0f, 349.59f, 281.51f, 383.12f, 248.05f, 394.43f, 268.15f, 1.0f, 402.18f, 281.92f, 369.75f, 321.42f, 316.78f, 330.65f)),
    UtyanPart("hand__bl", null, 0xFFFFFFFF.toInt(), 8.00f, false, floatArrayOf(0.0f, 215.87f, 277.17f, 1.0f, 227.4f, 278.15f, 242.08f, 276.41f, 251.54f, 276.33f),
        floatArrayOf(0.0f, 328.92f, 277.89f, 1.0f, 343.17f, 280.34f, 361.95f, 277.98f, 371.99f, 275.12f)),
)
