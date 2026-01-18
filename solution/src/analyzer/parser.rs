use super::counter::Counts;

pub fn process_line(line: &[u8], counts: &mut Counts) {
    let line = if let Some(b'\n') = line.last() {
        &line[..line.len() - 1]
    } else {
        line
    };

    let mut fields = [None; 4];
    let mut start = 0;
    let mut idx = 0;

    for (i, &b) in line.iter().enumerate() {
        if b == b'|' {
            if idx >= 4 {
                counts.malformed += 1;
                return;
            }
            fields[idx] = Some(&line[start..i]);
            idx += 1;
            start = i + 1;
        }
    }

    if idx != 3 {
        counts.malformed += 1;
        return;
    }

    fields[3] = Some(&line[start..]);

    match fields[1] {
        Some(b"INFO") => counts.info += 1,
        Some(b"WARN") => counts.warn += 1,
        Some(b"ERROR") => counts.error += 1,
        _ => counts.malformed += 1,
    }
}
