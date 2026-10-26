import Foundation

/// Where the Note of a day is (CONTEXT, Daily note): its folder from the Vault's root, and its
/// name written as the date in Obsidian's format, moment.js's tokens. Chosen in Settings; Noteki
/// does not read Obsidian's own (ADR 0002).
public struct DailyNote: Equatable, Sendable {
    public let folder: String
    public let format: String

    /// To match the user's Vault (SPEC, Daily note): `Journal/Daily_2025/2025-09-16.md`, a
    /// folder for each year.
    public static let standard = DailyNote(folder: "Journal", format: standardFormat)
    private static let standardFormat = "[Daily_]YYYY/YYYY-MM-DD"

    /// Slashes around the folder and blanks are dropped; an empty format is the standard one.
    public init(folder: String, format: String) {
        self.folder = folder.trimmingCharacters(in: CharacterSet(charactersIn: "/ "))
        let format = format.trimmingCharacters(in: .whitespaces)
        self.format = format.isEmpty ? Self.standardFormat : format
    }

    /// The Note of the day `date` falls on, in the Mac's time zone. A `/` in the format makes
    /// folders, as in Obsidian.
    public func note(on date: Date) -> Note {
        let name = Self.formatted(date, as: format)
        return Note(path: (folder.isEmpty ? name : folder + "/" + name) + "." + Note.fileExtension)
    }

    /// moment.js's tokens, longest first; anything else, text in `[brackets]` and a character
    /// after `\\` is written as it is. Names are English, moment's own default (not the language
    /// Obsidian may be set to). Not the localized formats (`L`, `LL`…).
    private static let tokens = [
        "YYYY", "GGGG", "gggg", "MMMM", "DDDD", "dddd", "DDDo", "MMM", "DDD", "ddd",
        "YY", "GG", "gg", "MM", "Mo", "Qo", "Do", "DD", "do", "dd", "Wo", "WW", "wo", "ww", "HH", "hh", "mm", "ss",
        "M", "Q", "D", "d", "E", "e", "W", "w", "H", "h", "m", "s", "A", "a",
    ]

    private static func formatted(_ date: Date, as format: String) -> String {
        var result = ""
        var rest = Substring(format)
        while let first = rest.first {
            if first == "[", let close = rest.dropFirst().firstIndex(where: { $0 == "]" || $0 == "[" }), rest[close] == "]" {
                result += rest[rest.index(after: rest.startIndex)..<close]
                rest = rest[rest.index(after: close)...]
            } else if first == "\\", rest.count > 1 {
                result.append(rest[rest.index(after: rest.startIndex)])
                rest = rest.dropFirst(2)
            } else if let token = tokens.first(where: { rest.hasPrefix($0) }) {
                result += value(of: token, for: date)
                rest = rest.dropFirst(token.count)
            } else {
                result.append(first)
                rest = rest.dropFirst()
            }
        }
        return result
    }

    private static func value(of token: String, for date: Date) -> String {
        var gregorian = Calendar(identifier: .gregorian)
        gregorian.locale = Locale(identifier: "en_US_POSIX")
        // moment's English weeks start on Sunday, its ISO weeks on Monday.
        gregorian.firstWeekday = 1
        gregorian.minimumDaysInFirstWeek = 1
        let iso = Calendar(identifier: .iso8601)
        let year = gregorian.component(.year, from: date)
        let month = gregorian.component(.month, from: date)
        let day = gregorian.component(.day, from: date)
        let weekday = gregorian.component(.weekday, from: date) - 1
        let dayOfYear = gregorian.ordinality(of: .day, in: .year, for: date) ?? 1
        let hour12 = (gregorian.component(.hour, from: date) + 11) % 12 + 1
        func padded(_ number: Int, _ width: Int) -> String {
            String(repeating: "0", count: max(0, width - String(number).count)) + String(number)
        }
        switch token {
        case "YYYY": return padded(year, 4)
        case "YY": return padded(year % 100, 2)
        case "GGGG": return padded(iso.component(.yearForWeekOfYear, from: date), 4)
        case "GG": return padded(iso.component(.yearForWeekOfYear, from: date) % 100, 2)
        case "gggg": return padded(gregorian.component(.yearForWeekOfYear, from: date), 4)
        case "gg": return padded(gregorian.component(.yearForWeekOfYear, from: date) % 100, 2)
        case "Q": return String((month - 1) / 3 + 1)
        case "Qo": return ordinal((month - 1) / 3 + 1)
        case "Mo": return ordinal(month)
        case "DDDo": return ordinal(dayOfYear)
        case "do": return ordinal(weekday)
        case "Wo": return ordinal(iso.component(.weekOfYear, from: date))
        case "wo": return ordinal(gregorian.component(.weekOfYear, from: date))
        case "E": return String(iso.component(.weekday, from: date) == 1 ? 7 : iso.component(.weekday, from: date) - 1)
        case "e": return String(weekday)
        case "HH": return padded(gregorian.component(.hour, from: date), 2)
        case "H": return String(gregorian.component(.hour, from: date))
        case "hh": return padded(hour12, 2)
        case "h": return String(hour12)
        case "mm": return padded(gregorian.component(.minute, from: date), 2)
        case "m": return String(gregorian.component(.minute, from: date))
        case "ss": return padded(gregorian.component(.second, from: date), 2)
        case "s": return String(gregorian.component(.second, from: date))
        case "A": return gregorian.component(.hour, from: date) < 12 ? "AM" : "PM"
        case "a": return gregorian.component(.hour, from: date) < 12 ? "am" : "pm"
        case "MMMM": return gregorian.monthSymbols[month - 1]
        case "MMM": return gregorian.shortMonthSymbols[month - 1]
        case "MM": return padded(month, 2)
        case "M": return String(month)
        case "DDDD": return padded(dayOfYear, 3)
        case "DDD": return String(dayOfYear)
        case "DD": return padded(day, 2)
        case "D": return String(day)
        case "Do": return ordinal(day)
        case "dddd": return gregorian.weekdaySymbols[weekday]
        case "ddd": return gregorian.shortWeekdaySymbols[weekday]
        case "dd": return String(gregorian.shortWeekdaySymbols[weekday].prefix(2))
        case "d": return String(weekday)
        case "WW": return padded(iso.component(.weekOfYear, from: date), 2)
        case "W": return String(iso.component(.weekOfYear, from: date))
        case "ww": return padded(gregorian.component(.weekOfYear, from: date), 2)
        case "w": return String(gregorian.component(.weekOfYear, from: date))
        default: return token
        }
    }

    /// 1st, 2nd, 3rd, 4th, 11th…
    private static func ordinal(_ number: Int) -> String {
        if (11...13).contains(number % 100) { return "\(number)th" }
        switch number % 10 {
        case 1: return "\(number)st"
        case 2: return "\(number)nd"
        case 3: return "\(number)rd"
        default: return "\(number)th"
        }
    }
}

extension Vault {
    /// Today's Note, or the Note of `date`: the one there, as it is named on disk, or a new empty
    /// one, its folders created. Refused in an Excluded folder, with a name Obsidian refuses or
    /// a Wikilink cannot hold, or a path that leaves its folder (`..`, an empty name).
    public func dailyNote(_ daily: DailyNote, on date: Date = .now) throws -> Note {
        let path = daily.note(on: date).path
        let components = path.split(separator: "/", omittingEmptySubsequences: false)
        guard !components.contains(where: { $0.isEmpty || $0 == "." || $0 == ".." || $0 == ".md" }),
              !path.contains(where: { "[]#^|".contains($0) }) else { throw CreateError.invalidName }
        do {
            return try createNote(for: path, from: nil)
        } catch CreateError.exists(let existing) {
            return existing
        }
    }
}
