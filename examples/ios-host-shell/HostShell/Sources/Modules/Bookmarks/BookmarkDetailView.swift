import SwiftUI

struct BookmarkDetailView: View {
    let row: [String]
    let columns: [String]

    var body: some View {
        List {
            ForEach(columns.indices, id: \.self) { i in
                if i < row.count {
                    LabeledContent(columns[i], value: row[i])
                }
            }
        }
        .navigationTitle(columnValue("title"))
    }

    private func columnValue(_ name: String) -> String {
        guard let idx = columns.firstIndex(of: name), idx < row.count else { return "" }
        return row[idx]
    }
}
