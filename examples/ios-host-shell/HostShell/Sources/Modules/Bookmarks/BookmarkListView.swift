import SwiftUI

struct BookmarkListView: View {
    @EnvironmentObject private var appState: AppState
    @State private var bookmarks: [[String]] = []
    @State private var columns: [String] = []
    @State private var showingAdd = false

    var body: some View {
        NavigationStack {
            List(bookmarks.indices, id: \.self) { i in
                let row = bookmarks[i]
                NavigationLink(destination: BookmarkDetailView(row: row, columns: columns)) {
                    VStack(alignment: .leading) {
                        Text(columnValue(row, "title"))
                            .font(.headline)
                        Text(columnValue(row, "url"))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .navigationTitle("Bookmarks")
            .toolbar {
                Button(action: { showingAdd = true }) {
                    Image(systemName: "plus")
                }
            }
            .sheet(isPresented: $showingAdd) {
                AddBookmarkView { reload() }
            }
            .onAppear { reload() }
        }
    }

    private func reload() {
        do {
            let result = try appState.driver.executeSql(
                sql: "SELECT id, title, url, category FROM bookmark"
            )
            columns = result.columns
            bookmarks = result.rows
        } catch {
            bookmarks = []
        }
    }

    private func columnValue(_ row: [String], _ name: String) -> String {
        guard let idx = columns.firstIndex(of: name), idx < row.count else { return "" }
        return row[idx]
    }
}

struct AddBookmarkView: View {
    @EnvironmentObject private var appState: AppState
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var url = ""
    let onSave: () -> Void

    var body: some View {
        NavigationStack {
            Form {
                TextField("Title", text: $title)
                TextField("URL", text: $url)
            }
            .navigationTitle("Add Bookmark")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(title.isEmpty || url.isEmpty)
                }
            }
        }
    }

    private func save() {
        do {
            _ = try appState.driver.executeSql(
                sql: "INSERT INTO bookmark (title, url) VALUES ('\(title)', '\(url)')"
            )
            onSave()
            dismiss()
        } catch {
            // Handle error in production
        }
    }
}
