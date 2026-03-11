import SwiftUI

struct ContactListView: View {
    @EnvironmentObject private var appState: AppState
    @State private var contacts: [[String]] = []
    @State private var columns: [String] = []
    @State private var showingAdd = false

    var body: some View {
        NavigationStack {
            List(contacts.indices, id: \.self) { i in
                let row = contacts[i]
                VStack(alignment: .leading) {
                    Text(columnValue(row, columns, "name"))
                        .font(.headline)
                    if !columnValue(row, columns, "email").isEmpty {
                        Text(columnValue(row, columns, "email"))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    if !columnValue(row, columns, "relationship").isEmpty {
                        Text(columnValue(row, columns, "relationship"))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                }
            }
            .navigationTitle("Contacts")
            .toolbar {
                Button(action: { showingAdd = true }) {
                    Image(systemName: "plus")
                }
            }
            .sheet(isPresented: $showingAdd) {
                AddContactView { reload() }
            }
            .onAppear { reload() }
        }
    }

    private func reload() {
        do {
            let result = try appState.driver.executeSql(
                sql: "SELECT id, name, relationship, email FROM contact"
            )
            columns = result.columns
            contacts = result.rows
        } catch {
            contacts = []
        }
    }

}

struct AddContactView: View {
    @EnvironmentObject private var appState: AppState
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var email = ""
    @State private var relationship = ""
    let onSave: () -> Void

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
                TextField("Email", text: $email)
                TextField("Relationship", text: $relationship)
            }
            .navigationTitle("Add Contact")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(name.isEmpty)
                }
            }
        }
    }

    private func save() {
        do {
            let n = name.replacingOccurrences(of: "'", with: "''")
            let e = email.replacingOccurrences(of: "'", with: "''")
            let r = relationship.replacingOccurrences(of: "'", with: "''")
            _ = try appState.driver.executeSql(
                sql: "INSERT INTO contact (name, email, relationship) VALUES ('\(n)', '\(e)', '\(r)')"
            )
            onSave()
            dismiss()
        } catch {
            // Handle error in production
        }
    }
}
