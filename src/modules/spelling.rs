//! Best-effort ASCII identifier repair with a fixed matching work bound.

/// At most 256 source-ordered eligible candidates, each at most 64 bytes.
/// The caller controls visibility. Equal distances use lexical order rather
/// than hash-map iteration; long names retain the ordinary repair guidance.
pub(super) fn spelling_hint<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<&'a str> {
    if name.len() > 64 {
        return None;
    }
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates
        .filter(|c| c.len() <= 64 && c.len().abs_diff(name.len()) <= 2)
        .take(256)
    {
        let mut previous = [0usize; 65];
        let mut next = [0usize; 65];
        for (i, cell) in previous.iter_mut().enumerate().take(candidate.len() + 1) {
            *cell = i;
        }
        for (i, left) in name.bytes().enumerate() {
            next[0] = i + 1;
            for (j, right) in candidate.bytes().enumerate() {
                next[j + 1] = (next[j] + 1)
                    .min(previous[j + 1] + 1)
                    .min(previous[j] + usize::from(left != right));
            }
            std::mem::swap(&mut previous, &mut next);
        }
        let distance = previous[candidate.len()];
        if distance <= 2 && best.map_or(true, |old| (distance, candidate) < old) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, name)| name)
}
