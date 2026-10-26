import { cpp } from "@codemirror/lang-cpp";
import { css } from "@codemirror/lang-css";
import { go } from "@codemirror/lang-go";
import { html } from "@codemirror/lang-html";
import { java } from "@codemirror/lang-java";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { sql } from "@codemirror/lang-sql";
import { yaml } from "@codemirror/lang-yaml";
import { type Language, LanguageDescription, LanguageSupport, StreamLanguage, type StringStream } from "@codemirror/language";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { swift } from "@codemirror/legacy-modes/mode/swift";

// The languages of fenced code coloured as such (SPEC, Live preview): the main ones, not all of
// CodeMirror's, which would make the editor several times larger. A fence naming another language
// gets a plain colouring of strings, numbers, comments and common keywords; a fence naming none is
// left uncoloured, as output or plain text usually is.

const described = (name: string, alias: string[], support: LanguageSupport) => LanguageDescription.of({ name, alias, support });

const languages = [
  described("JavaScript", ["js", "jsx", "mjs", "cjs", "node"], javascript({ jsx: true })),
  described("TypeScript", ["ts", "tsx", "mts", "cts"], javascript({ jsx: true, typescript: true })),
  described("Python", ["py", "python3"], python()),
  described("JSON", ["jsonc", "json5"], json()),
  described("HTML", ["htm", "xml", "svg", "xhtml"], html()),
  described("CSS", ["scss", "less"], css()),
  described("SQL", ["sqlite", "postgres", "postgresql", "mysql"], sql()),
  described("YAML", ["yml"], yaml()),
  described("Rust", ["rs"], rust()),
  described("Go", ["golang"], go()),
  described("Java", ["kotlin", "kt"], java()),
  described("C++", ["cpp", "c", "h", "hpp", "cc", "objc", "objective-c", "c#", "csharp", "cs"], cpp()),
  described("Swift", [], new LanguageSupport(StreamLanguage.define(swift))),
  described("Shell", ["sh", "bash", "zsh", "console", "shellsession"], new LanguageSupport(StreamLanguage.define(shell))),
];

const keywords = new Set(
  (
    "if else elif for while do loop return break continue switch case default match when try catch except finally " +
    "throw raise function func fn fun def class struct enum interface trait impl type module import export from use " +
    "package let var val const static public private protected new this self super true false null nil none undefined " +
    "and or not in is async await yield end then begin"
  ).split(" "),
);

/** The colouring of any other language: strings, numbers, `//`, `#`, `--` and block comments, common keywords. */
const plainCode = StreamLanguage.define<{ inComment: boolean }>({
  name: "code",
  startState: () => ({ inComment: false }),
  token(stream: StringStream, state) {
    if (state.inComment) {
      state.inComment = !stream.skipTo("*/");
      if (state.inComment) stream.skipToEnd();
      else stream.match("*/");
      return "comment";
    }
    if (stream.eatSpace()) return null;
    if (stream.match("/*")) {
      state.inComment = true;
      return "comment";
    }
    if (stream.match("//") || stream.match("--") || (stream.sol() || /\s/.test(stream.string.charAt(stream.pos - 1))) && stream.match("#")) {
      stream.skipToEnd();
      return "comment";
    }
    const quote = stream.peek();
    if (quote === '"' || quote === "'" || quote === "`") {
      stream.next();
      let escaped = false;
      for (let next = stream.next(); next !== undefined; next = stream.next()) {
        if (next === quote && !escaped) break;
        escaped = !escaped && next === "\\";
      }
      return "string";
    }
    if (stream.match(/^(?:0x[\da-f]+|\d[\d_]*(?:\.\d+)?(?:e[+-]?\d+)?)/i)) return "number";
    if (stream.match(/^[\p{L}_$][\p{L}\p{N}_$]*/u)) {
      const word = stream.current();
      if (keywords.has(word)) return "keyword";
      return /^\p{Lu}/u.test(word) ? "typeName" : null;
    }
    stream.next();
    return null;
  },
});

/** The language of a fence's info string, `swift` or `ts title="x"`: one of the main ones, the plain colouring, or none. */
export function codeLanguage(info: string): Language | LanguageDescription | null {
  const name = info.trim().split(/\s/)[0];
  if (!name) return null;
  return LanguageDescription.matchLanguageName(languages, name, false) ?? plainCode;
}
