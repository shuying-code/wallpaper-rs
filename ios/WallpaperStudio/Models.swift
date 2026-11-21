import Foundation

struct DailyWallpaper: Identifiable, Hashable {
    let id: String
    let title: String
    let copyright: String
    let date: String
    let thumbnailURL: URL
    let fullURL: URL

    var detail: WallpaperDetail {
        WallpaperDetail(
            id: id,
            title: title,
            subtitle: copyright,
            source: "必应每日壁纸",
            thumbnailURL: thumbnailURL,
            fullURL: fullURL,
            pageURL: nil
        )
    }
}

struct GalleryItem: Identifiable, Hashable {
    let id: String
    let title: String
    let thumbnailURL: URL
    let fullURL: URL
    let pageURL: URL?

    var detail: WallpaperDetail {
        WallpaperDetail(
            id: id,
            title: title.isEmpty ? "4K 壁纸" : title,
            subtitle: "必应 4K 图库 · 原图直出",
            source: "bing.ioliu.cn",
            thumbnailURL: thumbnailURL,
            fullURL: fullURL,
            pageURL: pageURL
        )
    }
}

struct WallpaperDetail: Identifiable, Hashable {
    let id: String
    let title: String
    let subtitle: String
    let source: String
    let thumbnailURL: URL
    let fullURL: URL
    let pageURL: URL?
}

struct GalleryPage {
    let items: [GalleryItem]
    let page: Int
    let hasMore: Bool
}
