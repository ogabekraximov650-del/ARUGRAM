package uz.arumediatv.com.data

import org.json.JSONObject

/**
 * UZ: Admin uchun ma'lumotlar bazasini FAQAT O'QISH (jurnallar = jadvallar, ustunlar, yozuvlar) — serverdagi `/api/admin/db/*` (`worker/src/dbview.rs`).
 * EN: read-only admin database browser client (tables, columns, rows).
 */
data class DbTable(val name: String, val kind: String)

data class DbColumn(val name: String, val type: String, val pk: Boolean)

/** Bitta sahifa: `rows[i][j]` — i-yozuvning j-ustuni (`null` — bo'sh qiymat). */
data class DbPage(val table: String, val columns: List<DbColumn>, val rows: List<List<String?>>, val offset: Int, val limit: Int, val hasMore: Boolean)

suspend fun AdminApi.dbTables(): Pair<List<DbTable>, String?> {
    val r = get("/api/admin/db/tables")
    val a = r.json?.optJSONArray("items")
    if (!r.ok || a == null) return emptyList<DbTable>() to errorOf(r, "Jurnallar olinmadi")
    return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { DbTable(it.optString("name"), it.optString("kind")) } to null
}

suspend fun AdminApi.dbRows(table: String, offset: Int, limit: Int = 30, order: String = "", desc: Boolean = true, col: String = "", q: String = ""): Pair<DbPage?, String?> {
    fun enc(s: String) = java.net.URLEncoder.encode(s, "UTF-8")
    val r = get("/api/admin/db/rows?table=${enc(table)}&offset=$offset&limit=$limit&order=${enc(order)}&dir=${if (desc) "desc" else "asc"}&col=${enc(col)}&q=${enc(q)}")
    val j: JSONObject = r.json ?: return null to errorOf(r, "Yozuvlar olinmadi")
    if (!r.ok) return null to errorOf(r, "Yozuvlar olinmadi")
    val cols = j.optJSONArray("columns")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { DbColumn(it.optString("name"), it.optString("type"), it.optBoolean("pk")) } }.orEmpty()
    val rowsJ = j.optJSONArray("rows")
    val rows = (0 until (rowsJ?.length() ?: 0)).map { i ->
        val row = rowsJ?.optJSONArray(i)
        (0 until cols.size).map { c ->
            val v = row?.opt(c)
            if (v == null || v == JSONObject.NULL) null else v.toString()
        }
    }
    return DbPage(j.optString("table"), cols, rows, j.optInt("offset"), j.optInt("limit", limit), j.optBoolean("has_more")) to null
}
