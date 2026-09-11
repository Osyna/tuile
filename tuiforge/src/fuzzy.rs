//! Fuzzy subsequence matching for command palettes, filters and pickers.

/// Best-scoring subsequence match (bonuses for adjacency, word starts, capitals).
/// Returns `(score, matched char positions)`; higher score is better. Case-insensitive.
pub fn fuzzy(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    let q: Vec<char> = query
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    if q.is_empty() {
        return Some((0, Vec::new()));
    }
    let hay: Vec<char> = text.chars().collect();
    let low: Vec<char> = hay
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    // memo[(qi, start)] = best match of q[qi..] inside hay[start..]; adjacency bonus applies when
    // the match lands exactly at `start` (the previous char matched at start - 1).
    // Outer Option is "not computed yet", inner is "no match"; a named type would hide that.
    #[allow(clippy::type_complexity)]
    let mut memo: Vec<Option<Option<(i32, Vec<usize>)>>> =
        vec![None; (q.len() + 1) * (hay.len() + 1)];
    type Memo = [Option<Option<(i32, Vec<usize>)>>];
    fn go(
        qi: usize,
        start: usize,
        q: &[char],
        hay: &[char],
        low: &[char],
        memo: &mut Memo,
    ) -> Option<(i32, Vec<usize>)> {
        if qi == q.len() {
            return Some((0, Vec::new()));
        }
        let key = qi * (hay.len() + 1) + start;
        if let Some(cached) = &memo[key] {
            return cached.clone();
        }
        let mut best: Option<(i32, Vec<usize>)> = None;
        for p in start..hay.len() {
            if low[p] != q[qi] {
                continue;
            }
            let Some((rest, mut positions)) = go(qi + 1, p + 1, q, hay, low, memo) else {
                continue;
            };
            let mut s = 1 + rest;
            if qi > 0 && p == start {
                s += 5;
            }
            if p == 0 || !hay[p - 1].is_alphanumeric() {
                s += 3;
            }
            if hay[p].is_uppercase() {
                s += 1;
            }
            if best.as_ref().is_none_or(|b| s > b.0) {
                positions.insert(0, p);
                best = Some((s, positions));
            }
        }
        memo[key] = Some(best.clone());
        best
    }
    let (score, positions) = go(0, 0, &q, &hay, &low, &mut memo)?;
    // shorter targets rank higher for equal matches
    Some((score - (hay.len() / 8) as i32, positions))
}

/// Rank `items` by fuzzy score against `query`, best first. Items that don't match are dropped.
/// Returns `(index, score, positions)`.
pub fn rank<'a, I>(query: &str, items: I) -> Vec<(usize, i32, Vec<usize>)>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut out: Vec<(usize, i32, Vec<usize>)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| fuzzy(query, s).map(|(sc, p)| (i, sc, p)))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_word_starts_and_adjacency() {
        let a = fuzzy("th", "Colour theme").unwrap().0;
        let b = fuzzy("th", "Width of the sidebar").unwrap().0;
        assert!(a > b);
        // greedy matching would take the `l` in "textual" and lose the contiguous run
        let light = fuzzy("light", "Theme: textual-light").unwrap();
        assert_eq!(light.1, vec![15, 16, 17, 18, 19]);
        assert!(light.0 > fuzzy("light", "Editor > Highlight current line").unwrap().0);
        assert!(fuzzy("zz", "theme").is_none());
        assert_eq!(fuzzy("", "x").unwrap().1, Vec::<usize>::new());
        let r = rank("st", ["reset", "Settings", "style"]);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2, 1, 0]);
    }
}
