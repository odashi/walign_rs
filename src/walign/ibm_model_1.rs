use crate::corpus::Corpus;
use anyhow::Result;
use byteorder::{LittleEndian, WriteBytesExt};
use ndarray::prelude::*;
use std::io::Write;

#[derive(Debug)]
pub struct Model {
    /// Translation probability from source word `f` to target word `e`.
    /// t_fe[(f, e)] = Pr(e|f)
    /// Shape: (|source_vocab|, |target_vocab|)
    pub t_fe: Array2<f64>,

    /// Transition probability from null word to target word `e`.
    /// t_e0[e] = Pr(e|NULL)
    /// Shape: (|target_vocab|,)
    pub t_0e: Array1<f64>,
}

impl Model {
    /// Saves a model to binary file.
    ///
    /// # File format
    ///
    /// - `f_size`: `u32`
    /// - `e_size`: `u32`
    /// - `t_fe`: `[f64; f_size * e_size]`
    /// - `t_0e`: `[f64; e_size]`
    ///
    /// All values are of little endian.
    /// `t_fe` and `t_0e` are stored in row-major ascending order.
    ///
    /// - `t_fe`: `[(0, 0)], [(0, 1)]..., [(0, el-1)], [(1, 0)], ...`
    /// - `t_0e`: `[0], [1], ...`
    pub fn save(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_u32::<LittleEndian>(self.t_fe.nrows() as u32)?;
        writer.write_u32::<LittleEndian>(self.t_fe.ncols() as u32)?;
        for &val in self.t_fe.iter() {
            writer.write_f64::<LittleEndian>(val)?;
        }
        for &val in self.t_0e.iter() {
            writer.write_f64::<LittleEndian>(val)?;
        }
        Ok(())
    }

    /// Trains IBM Model 1.
    pub fn train(corpus: &Corpus, iteration: u32) -> Self {
        let f_size = corpus.source_vocab.len();
        let e_size = corpus.target_vocab.len();

        eprintln!("Initializing model:");

        // Initializes probabilities with uniform PDF.
        let t_init = 1. / (e_size as f64 + 1.);
        let mut t_fe = Array2::<f64>::ones((f_size, e_size)) * t_init;
        let mut t_0e = Array1::<f64>::ones(e_size) * t_init;

        for epoch in 0..iteration {
            eprintln!("Epoch {}:", epoch + 1);

            // Corpus-wide probabilistic counts.
            // c_fe[(f, e)] = count(f, e)
            // c_0e[e]      = count(f=NULL, e)
            // c_f[f]       = count(f)         = sum_e count(f, e)
            // c_0          = count(f=NULL)    = sum_e count(f=NULL, e)
            let mut c_fe = Array2::<f64>::zeros((f_size, e_size));
            let mut c_0e = Array1::<f64>::zeros(e_size);
            let mut c_f = Array1::<f64>::zeros(f_size);
            let mut c_0 = 0f64;

            // Negative log-likelihood of the current model.
            let mut nll = 0f64;

            for (f_sent, e_sent) in corpus.source_sents.iter().zip(corpus.target_sents.iter()) {
                // Sentence-wise robabilistic counts for each target word type.
                let mut c_e = Array1::<f64>::zeros(e_size);
                // Likelihood of this sentence in terms of current model.
                let mut likelihood = 0f64;

                // Counts all alignment edges.
                for e in e_sent.iter().map(|&e| e as usize) {
                    // Source words.
                    for f in f_sent.iter().map(|&f| f as usize) {
                        let delta = t_fe[(f, e)];
                        c_e[e] += delta;
                        likelihood += delta;
                    }
                    // NULL word.
                    let delta = t_0e[e];
                    c_e[e] += delta;
                    likelihood += delta;
                }

                nll -= likelihood.log2() - e_size as f64 * ((f_size + 1) as f64).log2();

                // Update corpus-wide probabilistic counts.
                for e in e_sent.iter().map(|&e| e as usize) {
                    // Source words.
                    for f in f_sent.iter().map(|&f| f as usize) {
                        let delta = t_fe[(f, e)] / c_e[e];
                        c_fe[(f, e)] += delta;
                        c_f[f] += delta;
                    }
                    // NULL word.
                    let delta = t_0e[e] / c_e[e];
                    c_0e[e] += delta;
                    c_0 += delta;
                }
            }

            eprintln!("nll = {}", nll);

            // Update model.
            for e in 0..e_size {
                for f in 0..f_size {
                    t_fe[(f, e)] = if c_f[f] > 0f64 {
                        c_fe[(f, e)] / c_f[f]
                    } else {
                        0f64
                    };
                }
                t_0e[e] = if c_0 > 0f64 { c_0e[e] / c_0 } else { 0f64 }
            }
        }

        Self { t_fe, t_0e }
    }
}
