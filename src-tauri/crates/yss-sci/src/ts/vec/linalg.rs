fn cholesky_lower_in_place(a: &mut Array2<f64>) -> Result<(), ()> {
    let n = a.nrows();
    if a.ncols() != n {
        return Err(());
    }
    for j in 0..n {
        let mut s = 0.0;
        for k in 0..j {
            s += a[[j, k]].powi(2);
        }
        let d = a[[j, j]] - s;
        if d <= 0.0 {
            return Err(());
        }
        let ljj = d.sqrt();
        a[[j, j]] = ljj;
        for i in (j + 1)..n {
            let mut s = 0.0;
            for k in 0..j {
                s += a[[i, k]] * a[[j, k]];
            }
            a[[i, j]] = (a[[i, j]] - s) / ljj;
        }
        for i in 0..j {
            a[[i, j]] = 0.0;
        }
    }
    Ok(())
}
