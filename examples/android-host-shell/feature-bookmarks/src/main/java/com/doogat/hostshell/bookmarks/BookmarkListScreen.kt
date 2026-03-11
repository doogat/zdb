package com.doogat.hostshell.bookmarks

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.zdb_core.SqlResultRecord

@Composable
fun BookmarkListScreen(
    modifier: Modifier = Modifier,
    onQuery: (String) -> SqlResultRecord,
) {
    var rows by remember { mutableStateOf(emptyList<List<String>>()) }
    var columns by remember { mutableStateOf(emptyList<String>()) }

    LaunchedEffect(Unit) {
        val result = onQuery("SELECT id, title, url FROM bookmark")
        columns = result.columns
        rows = result.rows
    }

    LazyColumn(modifier = modifier) {
        items(rows.size) { i ->
            val row = rows[i]
            val title = columnValue(row, columns, "title")
            val url = columnValue(row, columns, "url")
            ListItem(
                headlineContent = { Text(title) },
                supportingContent = { Text(url) }
            )
            if (i < rows.size - 1) HorizontalDivider()
        }
    }
}

private fun columnValue(row: List<String>, columns: List<String>, name: String): String {
    val idx = columns.indexOf(name)
    return if (idx in row.indices) row[idx] else ""
}
