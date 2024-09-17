use std::cmp::min;

pub fn levenshtein(a: &str, b: &str) -> f64 {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    let mut matrix: Vec<Vec<usize>> = vec![vec![0; b_len + 1]; a_len + 1];

    for (i, item) in matrix.iter_mut().enumerate().take(a_len + 1) {
        item[0] = i;
    }

    for j in 0..(b_len + 1) {
        matrix[0][j] = j;
    }

    for (i, s_char) in a.chars().enumerate() {
        for (j, t_char) in b.chars().enumerate() {
            let substitution_cost = if s_char == t_char { 0 } else { 1 };
            let operations = [
                matrix[i][j + 1] + 1,             // deletion
                matrix[i + 1][j] + 1,             // insertion
                matrix[i][j] + substitution_cost, // substitution
            ];

            matrix[i + 1][j + 1] = *operations.iter().min().unwrap();

            // transposition
            if i > 0
                && j > 0
                && s_char == b.chars().nth(j - 1).unwrap()
                && t_char == a.chars().nth(i - 1).unwrap()
            {
                matrix[i + 1][j + 1] = min(
                    matrix[i + 1][j + 1],                     // cost without swappping
                    matrix[i - 1][j - 1] + substitution_cost, // cost with swapping
                );
            }
        }
    }

    let max = a_len.max(b_len) as f64;
    if max == 0.0 {
        return 0.0;
    }

    (matrix[a_len][b_len] as f64) / max
}
