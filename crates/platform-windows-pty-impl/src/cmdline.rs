//! Wiersz poleceń (reguły cudzysłowów MSVCRT) i blok środowiska UTF-16 dla `CreateProcessW`
//! — przenośne, testowane na każdej platformie.

/// Cytuje argument wg reguł `CommandLineToArgvW`/MSVCRT (ukośniki przed cudzysłowem podwajane).
fn quote(arg: &str, out: &mut String) {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\u{0b}', '"']) {
        out.push_str(arg);
        return;
    }
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            c => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
}

/// Wiersz poleceń: program (zawsze w cudzysłowie) + argumenty.
pub fn command_line(program: &str, args: &[String]) -> String {
    let mut out = String::new();
    out.push('"');
    out.push_str(program);
    out.push('"');
    for a in args {
        out.push(' ');
        quote(a, &mut out);
    }
    out
}

/// Blok środowiska `K=V\0…\0\0` (UTF-16), posortowany bez wielkości liter, bez duplikatów.
pub fn environment_block(env: &[(String, String)]) -> Vec<u16> {
    let mut vars: Vec<&(String, String)> = Vec::new();
    for pair in env {
        if !vars.iter().any(|(k, _)| k.eq_ignore_ascii_case(&pair.0)) {
            vars.push(pair);
        }
    }
    vars.sort_by_key(|(k, _)| k.to_uppercase());
    let mut block: Vec<u16> = Vec::new();
    for (k, v) in vars {
        block.extend(format!("{k}={v}").encode_utf16());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_follows_msvcrt() {
        let args: Vec<String> = ["login", "", "a b", r#"say "hi""#, r"C:\dir\", r#"a\"b"#]
            .map(String::from)
            .to_vec();
        assert_eq!(
            command_line(r"C:\Program Files\nodejs\claude.cmd", &args),
            r#""C:\Program Files\nodejs\claude.cmd" login "" "a b" "say \"hi\"" C:\dir\ "a\\\"b""#
        );
    }

    #[test]
    fn env_block_sorted_unique_terminated() {
        let env = vec![
            ("Path".to_owned(), "C:\\bin".to_owned()),
            ("APPDATA".to_owned(), "x".to_owned()),
            ("PATH".to_owned(), "dup".to_owned()),
        ];
        let block = environment_block(&env);
        let text = String::from_utf16_lossy(&block);
        assert_eq!(text, "APPDATA=x\0Path=C:\\bin\0\0");
        assert_eq!(environment_block(&[]), vec![0, 0]);
    }
}
