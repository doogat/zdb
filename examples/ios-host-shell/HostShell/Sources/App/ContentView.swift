import SwiftUI

struct ContentView: View {
    var body: some View {
        TabView {
            BookmarkListView()
                .tabItem {
                    Label("Bookmarks", systemImage: "bookmark")
                }

            ContactListView()
                .tabItem {
                    Label("Contacts", systemImage: "person.2")
                }

            SearchView()
                .tabItem {
                    Label("Search", systemImage: "magnifyingglass")
                }
        }
    }
}
