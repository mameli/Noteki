import Foundation
import Testing
import NotekiCore

@Suite("Daily note")
struct DailyNoteTests {
    /// Monday 3 March 2025 at 23:30, in the Mac's own time zone, as the day is counted.
    private let day = date(2025, 3, 3, hour: 23, minute: 30)

    @Test("today's Note is the date in the format, in the folder: by default Journal, a folder Daily_ for each year")
    func standard() {
        #expect(DailyNote.standard.note(on: day) == Note(path: "Journal/Daily_2025/2025-03-03.md"))
        #expect(DailyNote.standard.note(on: date(2026, 10, 26)) == Note(path: "Journal/Daily_2026/2026-10-26.md"))
        #expect(DailyNote(folder: "", format: "YYYY-MM-DD").note(on: day) == Note(path: "2025-03-03.md"))
    }

    @Test("the format is Obsidian's, moment.js's tokens: years, months, days, weekdays, weeks")
    func tokens() {
        let note = { (format: String) in DailyNote(folder: "", format: format).note(on: day).name }

        #expect(note("YY.M.D") == "25.3.3")
        #expect(note("dddd D MMMM YYYY") == "Monday 3 March 2025")
        #expect(note("ddd, MMM Do") == "Mon, Mar 3rd")
        #expect(note("GGGG-[W]WW") == "2025-W10")
        #expect(note("YYYY-DDDD") == "2025-062")
        #expect(note("Mo DDDo Qo do Wo") == "3rd 62nd 1st 1st 10th")
        #expect(note("YYYY-MM-DD hh.mm A") == "2025-03-03 11.30 PM")
        #expect(note("[Day] \\Do D") == "Day Do 3")
    }

    @Test("around the new year, ISO weeks belong to the year they mostly fall in")
    func newYear() {
        let note = { (format: String) in DailyNote(folder: "", format: format).note(on: date(2024, 12, 30)).name }

        #expect(note("GGGG-[W]WW") == "2025-W01")
        #expect(note("gggg-ww") == "2025-01")
        #expect(note("YYYY-MM-DD") == "2024-12-30")
    }

    @Test("text in brackets is kept as written, and a / makes folders, as in Obsidian")
    func literalsAndFolders() {
        #expect(DailyNote(folder: "Journal", format: "YYYY/MM/[Day] DD").note(on: day) == Note(path: "Journal/2025/03/Day 03.md"))
        #expect(DailyNote(folder: "/Journal/", format: "[Daily] YYYY-MM-DD").note(on: day) == Note(path: "Journal/Daily 2025-03-03.md"))
    }

    @Test("an empty format is the default one; the folder and format are trimmed")
    func blanks() {
        #expect(DailyNote(folder: " Journal ", format: "  ").note(on: day) == Note(path: "Journal/Daily_2025/2025-03-03.md"))
    }

    @Test("today's Note is created empty when missing, its folders too, and left as it is when there")
    func open() throws {
        let folder = try TemporaryVault(files: ["Journal/Daily_2025/2025-03-02.md": "yesterday"])
        let vault = Vault(root: folder.url)

        let created = try vault.dailyNote(DailyNote.standard, on: day)
        try folder.write("today", at: created.path)
        let again = try vault.dailyNote(DailyNote.standard, on: day)
        let elsewhere = try vault.dailyNote(DailyNote(folder: "New/Days", format: "YYYY-MM-DD"), on: day)

        #expect(created == Note(path: "Journal/Daily_2025/2025-03-03.md"))
        #expect(again == created)
        #expect(try folder.read(created.path) == "today")
        #expect(try folder.read(elsewhere.path) == "")
    }

    @Test("a Daily note found in other letter case is that Note; one in an Excluded folder is refused")
    func existingAndRefused() throws {
        let folder = try TemporaryVault(files: ["journal/daily_2025/2025-03-03.md": "kept"])
        let vault = Vault(root: folder.url)

        #expect(try vault.dailyNote(DailyNote.standard, on: day) == Note(path: "journal/daily_2025/2025-03-03.md"))
        #expect(throws: Vault.CreateError.outside) { try vault.dailyNote(DailyNote(folder: ".hidden", format: "YYYY"), on: day) }
    }

    @Test("a new Daily note in folders typed in other letter case has their names as on disk")
    func folderCase() throws {
        let folder = try TemporaryVault(files: ["Journal/Daily_2025/2025-03-02.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.dailyNote(DailyNote(folder: "journal/daily_2025", format: "YYYY-MM-DD"), on: day) == Note(path: "Journal/Daily_2025/2025-03-03.md"))
    }

    @Test("a format that leaves its folder, makes an empty name or holds what a Wikilink cannot is refused")
    func refusedFormats() throws {
        let folder = try TemporaryVault(files: ["Note.md": ""])
        let vault = Vault(root: folder.url)

        for format in ["[..]/YYYY", "YYYY/", "YYYY [#]1", "[a|b] YYYY"] {
            #expect(throws: Vault.CreateError.invalidName) { try vault.dailyNote(DailyNote(folder: "Journal", format: format), on: day) }
        }
    }
}

/// A day on the Gregorian calendar, in the Mac's own time zone, whatever calendar the Mac uses.
private func date(_ year: Int, _ month: Int, _ day: Int, hour: Int = 12, minute: Int = 0) -> Date {
    Calendar(identifier: .gregorian).date(from: DateComponents(year: year, month: month, day: day, hour: hour, minute: minute))!
}
