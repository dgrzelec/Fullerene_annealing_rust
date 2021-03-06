use std::{io::{Write, self, BufRead, BufReader}, collections::{VecDeque, HashSet}, ops::Index, f64::consts::PI, fs::File, path::Path, iter::Map};
use ndarray::{prelude::*, IndexLonger, AssignElem};
use rand::prelude::*;
use utilities::{save_gnuplot2D, save_gnuplot1D};

use crate::utilities::get_file_buffer;

mod utilities;

//################# params ###################
const R0: f64 = 1.315;
const R1: f64 = 1.7;
const R2: f64 = 2.0;
const De: f64 = 6.325;
const S: f64 = 1.29;
const lambda: f64 = 1.5;
const del: f64 = 0.80469;
const a0: f64 = 0.011304;
const c0: f64 = 19.;
const d0: f64 = 2.5;
// ##############################
type MatrixInt = Array2<i32>;
type VectorInt = Array1<i32>;
type VectorFloat = Array1<f64>;

// ############# structs and implementations
#[derive( Debug, Clone)]
struct Point6 {
    x: f64,
    y: f64,
    z: f64,
    r: f64,
    phi: f64,
    theta: f64
}

impl Point6 {
    fn new() -> Point6 {
        Point6 {x: 0., y: 0., z: 0., r: 0., phi: 0., theta: 0.}
    }

    fn from_cartesian<T: Index<usize, Output = f64>>(data: &T) -> Point6 {
        let xt:f64 = data[0];
        let yt = data[1];
        let zt = data[2];
        let rt = (xt.powi(2) + yt.powi(2) + zt.powi(2)).sqrt();
        Point6 { x: xt, 
                 y: yt, 
                 z: zt, 
                 r: rt, 
                 phi: (yt/xt).atan(), 
                 theta: (zt/rt).acos() }
    }

    fn from_spherical<T: Index<usize, Output = f64>>(data: &T) -> Point6 {
        let r = data[0];
        let phi = data[1];
        let theta = data[2];

        Point6 { x: r*theta.sin()*phi.cos(), 
                 y: r*theta.sin()*phi.sin(), 
                 z: r*theta.cos(), 
                 r, 
                 phi, 
                 theta }
    }
    // methods

    fn assert_angles(&mut self) {
        //phi [0, 2*PI]
        if self.phi < 0. { self.phi += 2.*PI}
        else if self.phi >2.*PI { self.phi -= 2.*PI  }

        //theta [0, PI]
        if self.theta < 0. { self.theta += PI}
        else if self.theta > PI { self.theta -= PI  }

    }
}

impl std::fmt::Display for Point6 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<10.5}\t{:<10.5}\t{:<10.5}\t{:<10.5}\t{:<10.5}\t{:<10.5}",
                 self.x, self.y, self.z, self.r, self.phi, self.theta)
    }
}

type Point6Array = Array1<Point6>;

#[derive( Debug)]
struct Fuleren {
    positions: Point6Array,
    size: usize,
    E: f64,
}

impl Fuleren {
    // constructors
    fn new(size: usize) -> Fuleren {
        Fuleren { positions: Point6Array::from_elem(size, Point6::new()),
                  size,
                  E: 0. }
    }
    
    fn from_file(path: &str) -> Result<Fuleren, String>  {
        
        if let Ok(lines) = read_lines(path) {
            let iter = lines
                                                    .map(|line| line
                                                        .expect("wrong line")
                                                        .split_ascii_whitespace()
                                                        .map(|num_str| num_str.parse::<f64>().expect("error duting parsing"))
                                                        .collect::<Array1<f64>>())
                                                    .map(|data| Point6::from_cartesian(&data));
            let pos_array: Point6Array = iter.collect();
        Ok(Fuleren {size: pos_array.len(), E: 0.,
                positions: pos_array} )
        }
        else {
            Err("Error during reading from file".to_string())
        }

    }

    // methods
    fn randomize_on_sphere(&mut self, r: f64) {
        let phi_distr = rand::distributions::Uniform::new_inclusive(0., 2.*PI);
        let theta_distr = rand::distributions::Uniform::new_inclusive(0., PI);
        let mut rng = rand::thread_rng();

        self.positions.iter_mut()
                      .for_each(|point| 
                                point.assign_elem(Point6::from_spherical(&[r, 
                                                                            rng.sample(phi_distr), 
                                                                            rng.sample(theta_distr)]) ));
    }

    fn random_atom_shift(&mut self, i: usize, beta: f64) -> bool {
        let mut rng = rand::thread_rng();
        let distr = rand::distributions::Uniform::<f64>::new_inclusive(0., 1.);
        // hard coded change rates
        let w_r = 1e-4;
        let w_phi = 0.05;
        let w_theta = 0.05;

        let u1 = rng.sample(distr);
        let u2 = rng.sample(distr);
        let u3 = rng.sample(distr);

        // let mut atom = &mut self.positions[i];

        //save old values, assign new
        let r_old = self.positions[i].r;
        let phi_old = self.positions[i].phi;
        let theta_old = self.positions[i].theta;
        
        let v_old = self._vi(i);
        
        let r_new = self.positions[i].r + self.positions[i].r*(2.*u1 - 1.) * w_r;
        let phi_new = self.positions[i].phi + self.positions[i].phi*(2.*u2 - 1.) * w_phi;
        let theta_new = self.positions[i].theta + self.positions[i].theta*(2.*u3 - 1.) * w_theta;

        self.positions[i].r = r_new;
        self.positions[i].phi = phi_new;
        self.positions[i].theta = theta_new;

        self.positions[i].assert_angles();
        
        let r_new = self.positions[i].r;
        let phi_new = self.positions[i].phi;
        let theta_new = self.positions[i].theta;

        self.positions[i].assign_elem(Point6::from_spherical(&array![r_new, phi_new, theta_new])); //this array macro is probably very slow

        let v_new = self._vi(i);

        let _exp = (-beta*(v_new - v_old)).exp();
        let p_acc = if _exp < 1. { _exp} else { 1.}; // possibly redundand if

        let u4 = rng.sample(distr);
        if u4 <= p_acc {
            true
        }
        else {
            self.positions[i].assign_elem(Point6::from_spherical(&array![r_old, phi_old, theta_old]));
            false
        }
    }

    fn random_global_r_shift(&mut self, beta: f64) -> bool {
        let mut rng = rand::thread_rng();
        let distr = rand::distributions::Uniform::<f64>::new_inclusive(0., 1.);
        
        // old atom positions
        let atoms_old_array = self.positions.clone();
        
        let e_old = self.energy_calc();

        //hard coded rate of change
        let w_all = 1e-4;

        // updating radius of all atoms + their x,y,z positions via from_spherical constructor
        let u1 = rng.sample(distr);
        let r_change = (1. + w_all*(2.*u1 - 1.));
        let iter = self.positions.iter_mut();
        for atom in iter {
            atom.assign_elem(Point6::from_spherical(&array![atom.r*r_change,
                                                                        atom.phi,
                                                                        atom.theta]) ); 
        }

        let e_new = self.energy_calc();

        let _exp = (-beta*(e_new - e_old)).exp();
        let p_acc = if _exp < 1. { _exp} else { 1.}; 

        let u2 = rng.sample(distr);
        if u2 <= p_acc {
            true //since every atom is already updated
        }
        else {
            self.positions.assign_elem(atoms_old_array);
            false
        }


    }

    fn energy_calc(&mut self) -> f64 {

        let E = 0.5 * (0..self.size)
                    .into_iter()
                    .map(|i| self._vi(i))
                    .sum::<f64>();
        
        self.E = E;
        E
    }

    fn _vi(&self, i:usize) -> f64 {
        let mut vi = 0.;

        // create enumerate iterator with i != j 
        let iter = self.positions.iter()
                        .enumerate()
                        .filter(|(j,atom_j)| *j != i);
        
        for (j, _) in iter { // possible: create closure f_cut istead of this ifs
            let r_ij = self._r_ij(i, j); 

            if r_ij <= R1 {
                vi += _v_r(r_ij) - 0.5*(self._b_ij(i, j) + self._b_ij(j, i)) * _v_a(r_ij)
            }
            else if r_ij <= R2 {
                vi += 0.5*(1. + ((r_ij - R1)/(R2-R1)*PI).cos() )*
                            (_v_r(r_ij) - 0.5*(self._b_ij(i, j) + self._b_ij(j, i)) * _v_a(r_ij))
            }
        }
        vi
    }

    fn _b_ij(&self,i:usize, j:usize) -> f64 {
        (1. + self._ksi_ij(i, j)).powf(-del)
    }

    fn _ksi_ij(&self, i: usize, j: usize) -> f64 {
        let mut ksi = 0.;

        // create enumerate iterator with k != i and != j 
        let iter = self.positions.iter()
                        .enumerate()
                        .filter(|(k,atom_k)| *k != i && *k != j);
        
        for (k, atom_k) in iter { // possible: create closure f_cut istead of this ifs
            let r_ik = self._r_ij(i, k); 

            if r_ik <= R1 {
                ksi += self._g_ijk(i, j, k)
            }
            else if r_ik <= R2 {
                ksi += 0.5*(1. + ((r_ik - R1)/(R2-R1)*PI).cos() ) * self._g_ijk(i, j, k)
            }
        }
        
        ksi
    }

    fn _r_ij(&self, i:usize, j:usize) -> f64 {
        // let vec_i = array![self.positions[i].x,self.positions[i].y,self.positions[i].z];
        // let vec_j = array![self.positions[j].x,self.positions[j].y,self.positions[j].z];
        let vec_ij = [self.positions[j].x - self.positions[i].x,
                                self.positions[j].y - self.positions[i].y,
                                self.positions[j].z - self.positions[i].z];
        _mod_arr(&vec_ij)
    }

    fn mean_r(&self) -> f64 {
        self.positions.iter()
                      .map(|point| point.r)
                      .sum::<f64>()/(self.size as f64)
    }

    fn _g_ijk(&self, i: usize, j: usize, k: usize) -> f64 {

        let vec_ij = [self.positions[j].x - self.positions[i].x,
                                self.positions[j].y - self.positions[i].y,
                                self.positions[j].z - self.positions[i].z];
        let vec_ik = [self.positions[k].x - self.positions[i].x,
                                self.positions[k].y - self.positions[i].y,
                                self.positions[k].z - self.positions[i].z];

        let cos_ijk = (vec_ij[0]*vec_ik[0] + vec_ij[1]*vec_ik[1] + vec_ij[2]*vec_ik[2])/_mod_arr(&vec_ij)/_mod_arr(&vec_ik);
        
        // modyfication to forbid 4-atom bindings
        if cos_ijk > 0. {
            20. // experimental value
        }
        else {
            a0*( 1. + c0.powi(2)/d0.powi(2) - c0.powi(2)/( d0.powi(2) + (1. + cos_ijk).powi(2) ) )
        }

        // a0*( 1. + c0.powi(2)/d0.powi(2) - c0.powi(2)/( d0.powi(2) + (1. + cos_ijk).powi(2) ) )
        
    }

    fn pcf(&self) -> VectorFloat {
        // hard coded number of bins
        let M: usize = 100;
        let mut pcf = VectorFloat::zeros(M);
        let r_sr = self.mean_r();
        let r_max = 2.5*r_sr;

        let dr = r_max/M as f64;
        
        for i in 0..self.size {
            for j in (i+1)..self.size {
                let r = self._r_ij(i, j);
                let m = (r/dr).floor() as usize;
                // safety if; this is potentially unsafe but assuming we know what we are doing its ok
                if m < M {
                    pcf[m] += 2.*4.*PI*r_sr.powi(2)/( (self.size.pow(2) as f64)*2.*PI*r*dr);
                }
            }
        }
        pcf
    }

    fn save_pos_xyz(&self, path: &str) {
        let iter = self.positions.iter();

        let mut f = get_file_buffer(path);

        for atom in iter{
            write!(f, "{:<10.5}\t{:<10.5}\t{:<10.5}\n", atom.x, atom.y, atom.z).expect("Error during saving");
        }
    }
}

impl std::fmt::Display for Fuleren {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut res = write!(f, "Fuleren with {} atoms, Energy: {:8.3}\n", self.size, self.E);
        res = write!(f, "{:<10.5}\t{:<10.5}\t{:<10.5}\t{:<10.5}\t{:<10.5}\t{:<10.5}\n", "x", "y", "z", "r", "phi", "theta");
        for point in self.positions.iter(){
            res = write!(f, "{}\n", *point);
        }
        res
    }
}



// ####################################
// ########### functions #############

// for Brenner potential
fn _v_r(r: f64) -> f64 {
    De/(S - 1.) * (-(2.*S).sqrt() * lambda * (r - R0)).exp()
}

fn _v_a(r: f64) -> f64 {
    De*S/(S - 1.) * (-(2./S).sqrt() * lambda * (r - R0)).exp()
}

fn _mod_vec(vec: &Array1<f64>) -> f64 {
    (vec[0].powi(2) + vec[1].powi(2) + vec[2].powi(2)).sqrt()
}
fn _mod_arr(vec: &[f64;3]) -> f64 {
    (vec[0].powi(2) + vec[1].powi(2) + vec[2].powi(2)).sqrt()
}

fn check_angles(mut phi: f64, mut theta: f64) -> (f64, f64) {
    //phi [0, 2*PI]
    if phi < 0. { phi += 2.*PI}
    else if phi >2.*PI { phi -= 2.*PI  }

    //theta [0, PI]
    if theta < 0. { theta += PI}
    else if theta > PI { theta -= PI  }

    (phi, theta)
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename).expect("cannot read the file");
    Ok(io::BufReader::new(file).lines())
}

fn get_beta(it: usize, it_max: usize, b_min: f64, b_max: f64, p: f64) -> f64 {
    b_min + (it as f64/it_max as f64).powf(p) * (b_max - b_min)
}

// ##################################

fn main() {
    
    // test for preprepared data
    // let mut F = Fuleren::from_file("data/atoms_test.dat").unwrap();
    // F.energy_calc();
    // println!("{}", F);
    
    // // task 2: simulation for unchanged brennner potential #################################
    // let N = 30;
    // let beta_min = 1.;
    // let beta_max = 100.; // try
    // let p = 2.;
    // let it_max: usize = 100_000;
    // // for saving #############
    // let save_step: usize = 100;
    // let mut e_array = VectorFloat::zeros(it_max/save_step);
    // let mut r_mean_array = VectorFloat::zeros(it_max/save_step);

    // //################

    // let mut F = Fuleren::new(N);
    // F.randomize_on_sphere(2.5);

    // for it in 0..it_max {
    //     let beta = get_beta(it, it_max, beta_min, beta_max, p);

    //     // random atom shifts
    //     for i in 0..N {
    //         F.random_atom_shift(i, beta);
    //     }
    //     //global radius shift
    //     F.random_global_r_shift(beta);

    //     if it % save_step == 0 {
    //         // println!("E={}, r_mean={}, it={}", F.E, F.mean_r(), it);
    //         e_array[it / save_step] = F.E;
    //         r_mean_array[it / save_step] = F.mean_r();
    //     }
        
    // }
    // // let mut f= get_file_buffer("energy_tab.txt");

    // save_gnuplot1D(&e_array, "plots/energy_tab.dat");
    // save_gnuplot1D(&r_mean_array, "plots/r_tab.dat");
    // save_gnuplot1D(&F.pcf(), "plots/pcf.dat");
    // F.save_pos_xyz("plots/atoms.dat");
    // println!("{}", F);
    // println!("r_sr = {}", F.mean_r());
    // println!("E/N = {}", F.E/F.size as f64);
    // // ################################################


    //#################################
        // task 5: simulation for changed brennner potential, for N in range 30,60 #################################
        let beta_min = 1.;
        let beta_max = 100.; // try
        let p = 2.;
        let it_max: usize = 100_000;
        // for saving #############
        let mut EN_tab = VectorFloat::zeros(31);
        //################
    
        for N in 30..=60 {

            let mut F = Fuleren::new(N);
            F.randomize_on_sphere(2.5);
        
            for it in 0..it_max {
                let beta = get_beta(it, it_max, beta_min, beta_max, p);
        
                // random atom shifts
                for i in 0..N {
                    F.random_atom_shift(i, beta);
                }
                //global radius shift
                F.random_global_r_shift(beta);
        
                
                
            }
            EN_tab[N-30] = F.E/N as f64;
            println!("N = {}; E/N = {}", N, F.E/N as f64);
        }

        save_gnuplot1D(&EN_tab, "plots/EN_tab");
    //#################################


    //########## TIMINGS #############################
    // let mut F = Fuleren::new(60);
    // F.randomize_on_sphere(1.);

    // let iter_max = 1000_000;
    // let start = std::time::Instant::now();
    // for _ in 0..iter_max {
    //     F._ksi_ij(1, 2);
    // }
    // let duration = start.elapsed().as_micros();
    // println!("Time mean: {} us", duration as f64/(iter_max as f64));

    // let mut F = Fuleren::new(60);
    // F.randomize_on_sphere(1.);

    
    // let start = std::time::Instant::now();
    // for _ in 0..iter_max {
    //     F._g_ijk_test(1, 2, 3);
    // }
    // let duration = start.elapsed().as_nanos();
    // println!("Time mean: {} ns", duration as f64/(iter_max as f64));

}

