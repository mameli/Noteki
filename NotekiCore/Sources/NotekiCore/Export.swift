import Foundation

// Export (SPEC, Export): the editor gives the Note as a page of HTML whose images keep the address
// it loads them from, `noteki-file://vault/<path>`. The page must stand alone, as a file to send
// or a PDF: each image of the Vault goes in as data, as large as the editor shows it.
extension Vault {
    /// `html` with every `noteki-file://vault/…` address of an image of the Vault replaced by the
    /// image's data; one that cannot be read (missing, outside the Vault) is left out.
    public func inliningImages(in html: String) -> String {
        let prefix = "noteki-file://vault/"
        var result = ""
        var rest = html[...]
        while let start = rest.range(of: prefix) {
            result += rest[..<start.lowerBound]
            let tail = rest[start.upperBound...]
            let end = tail.firstIndex(where: { $0 == "\"" || $0 == "'" || $0 == ")" || $0 == " " }) ?? tail.endIndex
            // The editor adds `?v=…` to load a changed image again.
            let encoded = tail[..<end].split(separator: "?", maxSplits: 1).first.map(String.init) ?? ""
            let path = encoded.removingPercentEncoding ?? encoded
            if let image = servedImage(at: path) {
                result += "data:\(image.mimeType);base64,\(image.data.base64EncodedString())"
            }
            rest = tail[end...]
        }
        return result + rest
    }
}
