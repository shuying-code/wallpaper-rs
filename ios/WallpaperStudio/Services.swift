import Foundation
import Photos
import UIKit

enum WallpaperError: LocalizedError {
    case badResponse(Int)
    case invalidResponse
    case emptyImage
    case invalidImage
    case photoAccessDenied
    case saveFailed(String)

    var errorDescription: String? {
        switch self {
        case .badResponse(let code):
            return "服务器返回 HTTP \(code)。"
        case .invalidResponse:
            return "服务器返回的数据格式不正确。"
        case .emptyImage:
            return "下载到的图片为空。"
        case .invalidImage:
            return "图片解码失败，换一张试试。"
        case .photoAccessDenied:
            return "没有相册写入权限。请到设置 → 隐私与安全性 → 照片中允许访问。"
        case .saveFailed(let message):
            return "保存失败：\(message)"
        }
    }
}

private enum HTTPSupport {
    static let userAgent =
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1"

    static func validate(_ response: URLResponse) throws {
        guard let http = response as? HTTPURLResponse else {
            throw WallpaperError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            throw WallpaperError.badResponse(http.statusCode)
        }
    }
}

struct BingDailyService {
    private struct Response: Decodable {
        let images: [Image]
    }

    private struct Image: Decodable {
        let url: String?
        let urlbase: String?
        let title: String?
        let copyright: String?
        let startdate: String?
    }

    func fetch(count: Int = 8) async throws -> [DailyWallpaper] {
        var components = URLComponents(string: "https://cn.bing.com/HPImageArchive.aspx")!
        components.queryItems = [
            URLQueryItem(name: "format", value: "js"),
            URLQueryItem(name: "idx", value: "0"),
            URLQueryItem(name: "n", value: String(max(1, min(count, 8)))),
            URLQueryItem(name: "mkt", value: "zh-CN")
        ]
        guard let url = components.url else {
            throw WallpaperError.invalidResponse
        }

        var request = URLRequest(url: url)
        request.setValue(HTTPSupport.userAgent, forHTTPHeaderField: "User-Agent")
        request.setValue("https://cn.bing.com/", forHTTPHeaderField: "Referer")

        let (data, response) = try await URLSession.shared.data(for: request)
        try HTTPSupport.validate(response)
        let payload = try JSONDecoder().decode(Response.self, from: data)

        return payload.images.compactMap { image in
            guard let path = image.url, !path.isEmpty else { return nil }
            let base = URL(string: "https://cn.bing.com/")!
            guard let thumbnail = URL(string: path, relativeTo: base)?.absoluteURL else {
                return nil
            }

            let fullPath: String
            if let urlbase = Self.imageBase(image.urlbase) {
                fullPath = "/th?id=\(urlbase)_UHD.jpg"
            } else {
                let pathWithoutQuery = path.split(separator: "&").first.map(String.init) ?? path
                fullPath = pathWithoutQuery.replacingOccurrences(of: "_1920x1080", with: "_UHD")
            }
            guard let fullURL = URL(string: fullPath, relativeTo: base)?.absoluteURL else {
                return nil
            }

            let date = image.startdate ?? ""
            let title = (image.title?.isEmpty == false ? image.title! : "必应每日壁纸")
            return DailyWallpaper(
                id: "\(date)-\(image.urlbase ?? path)",
                title: title,
                copyright: image.copyright ?? "Microsoft Bing",
                date: date,
                thumbnailURL: thumbnail,
                fullURL: fullURL
            )
        }
    }

    private static func imageBase(_ raw: String?) -> String? {
        guard var value = raw?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else {
            return nil
        }
        if let range = value.range(of: "id=") {
            value = String(value[range.upperBound...])
        }
        let base = value.split(separator: "&").first.map(String.init) ?? value
        return base.isEmpty ? nil : base
    }
}

struct BingGalleryService {
    private let base = URL(string: "https://bing.ioliu.cn")!
    private let session: URLSession

    private struct Challenge: Decodable {
        let nonce: String
        let token: String
    }

    private struct Response: Decodable {
        let items: [Item]
        let page: Int?
        let pageSize: Int?
        let total: Int?
        let hasMore: Bool?
    }

    private struct Item: Decodable {
        let slug: String?
        let urlbase: String?
        let title: String?
    }

    init() {
        let configuration = URLSessionConfiguration.default
        configuration.httpCookieStorage = HTTPCookieStorage.shared
        configuration.httpShouldSetCookies = true
        configuration.timeoutIntervalForRequest = 20
        configuration.timeoutIntervalForResource = 40
        session = URLSession(configuration: configuration)
    }

    func list(page: Int, query: String) async throws -> GalleryPage {
        var components = URLComponents(
            url: base.appendingPathComponent("api/wallpapers"),
            resolvingAgainstBaseURL: false
        )!
        var items = [
            URLQueryItem(name: "page", value: String(max(1, page))),
            URLQueryItem(name: "pageSize", value: "24")
        ]
        if !query.isEmpty {
            items.append(URLQueryItem(name: "q", value: query))
        }
        components.queryItems = items
        guard let url = components.url else {
            throw WallpaperError.invalidResponse
        }

        var lastError: Error?
        for attempt in 0..<3 {
            do {
                let data = try await load(url: url)
                return try parse(data: data, requestedPage: page)
            } catch {
                lastError = error
                try? await Task.sleep(nanoseconds: 800_000_000)
            }
        }
        throw lastError ?? WallpaperError.invalidResponse
    }

    private func load(url: URL) async throws -> Data {
        let challengeURL = base.appendingPathComponent("api/public/challenge")
        var challengeRequest = URLRequest(url: challengeURL)
        challengeRequest.setValue("https://bing.ioliu.cn/", forHTTPHeaderField: "Referer")
        challengeRequest.setValue(HTTPSupport.userAgent, forHTTPHeaderField: "User-Agent")
        let (challengeData, challengeResponse) = try await session.data(for: challengeRequest)
        try HTTPSupport.validate(challengeResponse)

        if let http = challengeResponse as? HTTPURLResponse {
            let fields = http.allHeaderFields.reduce(into: [String: String]()) { result, pair in
                if let key = pair.key as? String {
                    result[key] = String(describing: pair.value)
                }
            }
            HTTPCookie.cookies(withResponseHeaderFields: fields, for: challengeURL).forEach {
                HTTPCookieStorage.shared.setCookie($0)
            }
        }

        let challenge = try JSONDecoder().decode(Challenge.self, from: challengeData)
        var request = URLRequest(url: url)
        request.setValue("https://bing.ioliu.cn/", forHTTPHeaderField: "Referer")
        request.setValue(HTTPSupport.userAgent, forHTTPHeaderField: "User-Agent")
        request.setValue(challenge.nonce, forHTTPHeaderField: "x-bf-nonce")
        request.setValue(challenge.token, forHTTPHeaderField: "x-bf-token")
        request.setValue(
            String(Int(Date().timeIntervalSince1970 * 1000)),
            forHTTPHeaderField: "x-bf-ts"
        )
        if let cookies = HTTPCookieStorage.shared.cookies(for: url), !cookies.isEmpty {
            request.setValue(
                cookies.map { "\($0.name)=\($0.value)" }.joined(separator: "; "),
                forHTTPHeaderField: "Cookie"
            )
        }

        let (data, response) = try await session.data(for: request)
        try HTTPSupport.validate(response)
        return data
    }

    private func parse(data: Data, requestedPage: Int) throws -> GalleryPage {
        let payload = try JSONDecoder().decode(Response.self, from: data)
        let baseURL = URL(string: "https://cn.bing.com")!
        let items = payload.items.enumerated().compactMap { index, item -> GalleryItem? in
            guard let urlbase = item.urlbase, !urlbase.isEmpty else { return nil }
            let fullPath: String
            let thumbnailPath: String
            if urlbase.hasPrefix("OHR.") {
                fullPath = "/th?id=\(urlbase)_UHD.jpg"
                thumbnailPath = "/th?id=\(urlbase)_1920x1080.jpg&w=800&h=450"
            } else {
                fullPath = "/th?id=\(urlbase)&rs=2&c=1&w=3840&h=2160"
                thumbnailPath = "/th?id=\(urlbase)&rs=2&c=1&w=800&h=450"
            }
            guard
                let fullURL = URL(string: fullPath, relativeTo: baseURL)?.absoluteURL,
                let thumbnailURL = URL(string: thumbnailPath, relativeTo: baseURL)?.absoluteURL
            else {
                return nil
            }
            let slug = item.slug ?? "\(requestedPage)-\(index)"
            return GalleryItem(
                id: slug,
                title: item.title ?? "",
                thumbnailURL: thumbnailURL,
                fullURL: fullURL,
                pageURL: URL(string: "https://bing.ioliu.cn/wallpapers/\(slug)")
            )
        }

        let pageSize = max(1, payload.pageSize ?? 24)
        let total = payload.total ?? 0
        let fallbackHasMore = requestedPage * pageSize < total
        return GalleryPage(
            items: items,
            page: payload.page ?? requestedPage,
            hasMore: payload.hasMore ?? fallbackHasMore
        )
    }
}

enum ImageDownloader {
    static func data(from url: URL) async throws -> Data {
        var request = URLRequest(url: url)
        request.setValue(HTTPSupport.userAgent, forHTTPHeaderField: "User-Agent")
        request.setValue("https://cn.bing.com/", forHTTPHeaderField: "Referer")
        let (data, response) = try await URLSession.shared.data(for: request)
        try HTTPSupport.validate(response)
        guard !data.isEmpty else {
            throw WallpaperError.emptyImage
        }
        return data
    }
}

enum PhotoLibrarySaver {
    static func save(data: Data) async throws {
        let status = await withCheckedContinuation { continuation in
            PHPhotoLibrary.requestAuthorization(for: .addOnly) { status in
                continuation.resume(returning: status)
            }
        }
        guard status == .authorized || status == .limited else {
            throw WallpaperError.photoAccessDenied
        }
        guard let image = UIImage(data: data) else {
            throw WallpaperError.invalidImage
        }

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            PHPhotoLibrary.shared().performChanges {
                PHAssetChangeRequest.creationRequestForAsset(from: image)
            } completionHandler: { success, error in
                if success {
                    continuation.resume()
                } else {
                    continuation.resume(
                        throwing: WallpaperError.saveFailed(error?.localizedDescription ?? "未知错误")
                    )
                }
            }
        }
    }
}
