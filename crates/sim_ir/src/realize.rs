//! Transfer function realization (controllable canonical form, = MATLAB/SciPy tf2ss).
use sim_core::expr::Matrix;

/// Returns (A, B, C, D).
pub fn tf_to_ss(num: &[f64], den: &[f64]) -> Result<(Matrix, Vec<f64>, Vec<f64>, f64), String> {
    let den: Vec<f64> = den.iter().copied().skip_while(|v| *v == 0.0).collect();
    if den.is_empty() {
        return Err("denominator must not be zero".into());
    }
    let mut num: Vec<f64> = num.iter().copied().skip_while(|v| *v == 0.0).collect();
    if num.is_empty() {
        num = vec![0.0];
    }
    if num.len() > den.len() {
        return Err(
            "transfer function must be proper (numerator degree <= denominator degree)".into(),
        );
    }
    let n = den.len() - 1;
    let a: Vec<f64> = den.iter().map(|v| v / den[0]).collect();
    let mut b = vec![0.0; n + 1 - num.len()];
    b.extend(num.iter().map(|v| v / den[0]));
    if n == 0 {
        return Ok((Matrix::empty(), vec![], vec![], b[0]));
    }
    let mut am = vec![0.0; n * n];
    for j in 0..n {
        am[j] = -a[j + 1];
    }
    for i in 1..n {
        am[i * n + (i - 1)] = 1.0;
    }
    let mut bv = vec![0.0; n];
    bv[0] = 1.0;
    let c: Vec<f64> = (0..n).map(|j| b[j + 1] - b[0] * a[j + 1]).collect();
    Ok((
        Matrix {
            rows: n,
            cols: n,
            data: am,
        },
        bv,
        c,
        b[0],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(n: usize, d: &[f64]) -> Matrix {
        Matrix {
            rows: n,
            cols: n,
            data: d.to_vec(),
        }
    }
    #[test]
    fn tf2ss_table() {
        assert_eq!(
            tf_to_ss(&[1.0], &[1.0, 1.0]).unwrap(),
            (m(1, &[-1.0]), vec![1.0], vec![1.0], 0.0)
        );
        assert_eq!(
            tf_to_ss(&[1.0], &[1.0, 3.0, 2.0]).unwrap(),
            (
                m(2, &[-3.0, -2.0, 1.0, 0.0]),
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                0.0
            )
        );
        assert_eq!(
            tf_to_ss(&[1.0, 0.0], &[1.0, 1.0]).unwrap(),
            (m(1, &[-1.0]), vec![1.0], vec![-1.0], 1.0)
        );
        assert_eq!(
            tf_to_ss(&[2.0], &[2.0, 4.0]).unwrap(),
            (m(1, &[-2.0]), vec![1.0], vec![1.0], 0.0)
        );
        assert_eq!(
            tf_to_ss(&[3.0], &[2.0]).unwrap(),
            (Matrix::empty(), vec![], vec![], 1.5)
        );
        assert_eq!(
            tf_to_ss(&[0.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).unwrap(),
            (m(1, &[-1.0]), vec![1.0], vec![1.0], 0.0)
        );
        assert!(tf_to_ss(&[1.0, 2.0, 3.0], &[1.0, 1.0])
            .unwrap_err()
            .contains("proper"));
        assert!(tf_to_ss(&[1.0], &[0.0]).unwrap_err().contains("zero"));
    }
}
