import Combine
import Foundation

@MainActor
final class WallpaperStore: ObservableObject {
    @Published private(set) var daily: [DailyWallpaper] = []
    @Published private(set) var gallery: [GalleryItem] = []
    @Published var query = ""
    @Published private(set) var isLoading = false
    @Published private(set) var isLoadingMore = false
    @Published private(set) var hasMore = false
    @Published var errorMessage: String?

    private let dailyService = BingDailyService()
    private let galleryService = BingGalleryService()
    private var dailyLoaded = false
    private var nextPage = 1

    func loadDailyIfNeeded() async {
        guard !dailyLoaded else { return }
        await refreshDaily()
    }

    func refreshDaily() async {
        isLoading = true
        defer { isLoading = false }
        do {
            daily = try await dailyService.fetch(count: 8)
            dailyLoaded = true
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func loadGalleryIfNeeded() async {
        guard gallery.isEmpty else { return }
        await search()
    }

    func search() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let page = try await galleryService.list(
                page: 1,
                query: query.trimmingCharacters(in: .whitespacesAndNewlines)
            )
            gallery = page.items
            nextPage = page.page + 1
            hasMore = page.hasMore
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func loadMore() async {
        guard hasMore, !isLoading, !isLoadingMore else { return }
        isLoadingMore = true
        defer { isLoadingMore = false }
        do {
            let page = try await galleryService.list(
                page: nextPage,
                query: query.trimmingCharacters(in: .whitespacesAndNewlines)
            )
            let known = Set(gallery.map(\.id))
            gallery.append(contentsOf: page.items.filter { !known.contains($0.id) })
            nextPage = page.page + 1
            hasMore = page.hasMore
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
