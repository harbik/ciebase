/*!
# Color Rendering Index Calculation

# References
- CIE 013.3-1995 Method of measuring and specifying colour rendering properties of light sources
  
 */

use std::sync::{LazyLock, OnceLock};
use nalgebra::{ArrayStorage, SMatrix};
use wasm_bindgen::prelude::*;


use crate::{CmtError, Colorant, Spectrum, RgbSpace, Illuminant, CIE1931, XYZ, Light};

/// Nummer of Test Color Sample Spectra
const N_TCS: usize = 14;

/// Create Array of Test Color Sample Spectra, as used to assess the color rendering performance of a light source.
///
/// The data for these is obtained from the <https:://cie.co.at> site's dataset library, on Sep 4, 2024.
/// The dataset uses a 360-830-5nm domain, included below in the 'TCS5' static matrix truncated to a domain from 380-780-5nm.
/// Here the dataset is converted to an array of 14 Spectra, using linear interpolation.
pub static TCS: LazyLock<[Colorant;N_TCS]> = LazyLock::new(|| {
        let s_vec: Vec<Colorant> = TCS5.column_iter().map(|s|
            Colorant(Spectrum::linear_interpolate(&[380.0, 780.0],s.as_slice()).unwrap())).collect();
        s_vec.try_into().unwrap()
    }
);

#[test]
fn tcs_test(){
    for (i,s) in TCS.iter().enumerate() {
        let xyz = CIE1931.xyz(&crate::StdIlluminant::D65, Some(s));
        let [r,g,b] = xyz.srgb();
        println!("{:2} ({r:3},{g:3},{b:3})", i+1);
    }
    
    let labs = [
        [50.1,38.3,14.1], [51.6,-21.6,24.7], [51.2,-20.7,16.1], [52.4,-7.9,21.4],
        [51.5,-27.8,-8.2], [51.0,-18.7,-24.4], [51.2,7.4,-30.7], [51.5,28.1,-8.4],
        [40.2,53.3,25.5], [56.5,-10.5,58.1], [40.9,-46.6,17.6], [25.3,9.2,-43.5],
        [60.0,14.3,12.3], [49.8,-18.6,28.8], [66.9,18.8,14.5]
    ];

}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
/// Encapcsulated Array of calculated Ri values, from a test light source.
pub struct CRI([f64;N_TCS]);

/// CRI calculation.
/// 
/// Can fail, for example if the Spectrum's correlated color temperature is out of range.
/// Uses CIE1931, and requires "cct"-feature.
impl TryFrom<&Illuminant> for CRI {
    type Error = CmtError;

    fn try_from(illuminant: &Illuminant) -> Result<Self, Self::Error> {
        let illuminant = &illuminant.clone().set_illuminance(&CIE1931, 100.0);
        // Calculate Device Under Test (dut) XYZ illuminant and sample values
        let xyz_dut = CIE1931.xyz_from_spectrum(illuminant, None);
        let xyz_dut_samples: [XYZ; N_TCS] = 
            TCS
                .iter()
               // .map(|colorant|CIE1931.xyz_of_sample_with_illuminant(illuminant, colorant))
                .map(|colorant|CIE1931.xyz(illuminant, Some(colorant)))
                .collect::<Vec<XYZ>>()
                .try_into().unwrap();

        // Determine reference color temperarture value
        let cct_dut = xyz_dut.cct()?.t();
        //println!("cct dut {cct_dut}");
        let illuminant_ref = if cct_dut <= 5000.0 {
            Illuminant::planckian(cct_dut).set_illuminance(&CIE1931, 100.0)
        } else {
            Illuminant::d_illuminant(cct_dut)?.set_illuminance(&CIE1931, 100.0)
        };

        // Calculate the reference illuminant values
        let xyz_ref = CIE1931.xyz_from_spectrum(&illuminant_ref, None);
        let xyz_ref_samples: [XYZ; N_TCS] = 
            TCS
                .iter()
              //  .map(|sample|CIE1931.xyz_of_sample_with_illuminant(&illuminant_ref, sample))
                .map(|sample|CIE1931.xyz(&illuminant_ref, Some(sample)))
                .collect::<Vec<XYZ>>()
                .try_into().unwrap();
        

        let cdt = cd(xyz_dut.uv60());
        let cdr = cd(xyz_ref.uv60());
        
        let ri : [f64; N_TCS] =
            xyz_ref_samples
                .iter()
                .zip(xyz_dut_samples.iter())
                .map(|(xyzr,xyz)|{
                    let cdti = cd(xyz.uv60());
                    let uv_vk = uv_kries(cdt, cdr, cdti);
                    let xyz_vk = XYZ::try_from_luv60( uv_vk[0], uv_vk[1], Some(xyz.xyz.unwrap().y), None).unwrap();
                    let uvw = xyz_vk.uvw64(xyz_ref);
                    let uvwr = xyzr.uvw64(xyz_ref);
                    100.0 - 4.6 * ((uvw[0] - uvwr[0]).powi(2) + (uvw[1] - uvwr[1]).powi(2) + (uvw[2] - uvwr[2]).powi(2)).sqrt()
                }).collect::<Vec<f64>>().try_into().unwrap();

        Ok(CRI(ri))
    }
}

impl AsRef<[f64]> for CRI {
    fn as_ref(&self) -> &[f64] {
       &self.0
    }
}

impl CRI {
    pub fn try_new(s: &Illuminant) -> Result<Self, CmtError> {
        s.try_into()
    }
}

// JS-WASM Interface code

#[cfg(target_arch="wasm32")] 
#[wasm_bindgen]
impl CRI {


}

#[cfg(test)]
mod cri_test {
    use crate::{cri::N_TCS, StdIlluminant, CRI, D50};

    #[test]
    fn cri_d50(){
        // should be all 100.0
        let cri0: CRI = (&D50).try_into().unwrap();
       // println!("{cri0:?}");
        approx::assert_ulps_eq!(
            cri0.as_ref(), 
            [100.0;N_TCS].as_ref(), epsilon = 0.03);
    }

    #[test]
    fn cri_f1(){
        // should be all 100.0
        let cri0: CRI = StdIlluminant::F1.illuminant().try_into().unwrap();
        println!("{cri0:?}");
       // approx::assert_ulps_eq!(cri0.as_ref(), [100.0;crate::cri::N_TCS].as_ref(), epsilon = 0.05);
    }

    #[test]
    fn cri_f3_1(){
        // 2932K, check with values as given in CIE15:2004 Table T.8.2
        let cri0: CRI = StdIlluminant::F3_1.illuminant().try_into().unwrap();
        approx::assert_ulps_eq!(
            cri0.as_ref(), 
            [42,69,89,39,41,52,66,13,-109,29,19,21,47,93].map(|v|v as f64).as_ref(), 
            epsilon = 1.0
        );
    }

    #[test]
    fn cri_f3_11(){
        // 5854K, check with values as given in CIE15:2004 Table T.8.2
        let cri0: CRI = StdIlluminant::F3_11.illuminant().try_into().unwrap();
        approx::assert_ulps_eq!(
            cri0.as_ref(), 
            [90,86,49,82,81,70,85,79,24,34,64,50,90,67].map(|v|v as f64).as_ref(), 
            epsilon = 1.0
        );
    }
}

fn cd(uv60: [f64;2]) -> [f64;2] {
    let [u,v] = uv60;
    [(4.0 - u - 10.0 * v) / v, (1.708 * v - 1.481 * u + 0.404) / v]
}

fn uv_kries(cdt: [f64;2], cdr: [f64;2], cdti: [f64;2] ) -> [f64;2] {
    let [ct, dt] = cdt;
    let [cr, dr] = cdr;
    let [cti, dti] = cdti;
    let den = 16.518 + 1.481 * (cr / ct) * cti - (dr / dt) * dti;
    [ (10.872 + 0.404 * (cr / ct) * cti - 4.0 * (dr / dt) * dti) / den, 5.520 / den]    
}

/// Spectral radiance factors of 14 test samples for the CIE colour rendering index calculation.
/// 
/// The data is defined over a wavelength domain of 380-780-5nm, and was downloaded from CIE
/// <http://cie.co.at>, public datasets, and truncated from 360 to 830nm range.
/// Data license: [CC BY-SA 4](https://creativecommons.org/licenses/by-sa/4.0/).
/// This data is used to generate an arra of 14 [Spectra], as used in this library, by interpolation
/// into a domain 380-780-1nm.
static TCS5: SMatrix::<f64, 81, N_TCS> = SMatrix::from_array_storage( ArrayStorage([
    [0.219, 0.239, 0.252, 0.256, 0.256, 0.254, 0.252, 0.248, 0.244, 0.24, 0.237, 0.232, 0.23, 0.226,
    0.225, 0.222, 0.22, 0.218, 0.216, 0.214, 0.214, 0.214, 0.216, 0.218, 0.223, 0.225, 0.226, 0.226,
    0.225, 0.225, 0.227, 0.23, 0.236, 0.245, 0.253, 0.262, 0.272, 0.283, 0.298, 0.318, 0.341, 0.367,
    0.39, 0.409, 0.424, 0.435, 0.442, 0.448, 0.45, 0.451, 0.451, 0.451, 0.451, 0.451, 0.45, 0.45,
    0.451, 0.451, 0.453, 0.454, 0.455, 0.457, 0.458, 0.46, 0.462, 0.463, 0.464, 0.465, 0.466, 0.466,
    0.466, 0.466, 0.467, 0.467, 0.467, 0.467, 0.467, 0.467, 0.467, 0.467, 0.467],
    [0.07, 0.079, 0.089, 0.101, 0.111, 0.116, 0.118, 0.12, 0.121, 0.122, 0.122, 0.122, 0.123, 0.124,
    0.127, 0.128, 0.131, 0.134, 0.138, 0.143, 0.15, 0.159, 0.174, 0.19, 0.207, 0.225, 0.242, 0.253,
    0.26, 0.264, 0.267, 0.269, 0.272, 0.276, 0.282, 0.289, 0.299, 0.309, 0.322, 0.329, 0.335, 0.339,
    0.341, 0.341, 0.342, 0.342, 0.342, 0.341, 0.341, 0.339, 0.339, 0.338, 0.338, 0.337, 0.336,
    0.335, 0.334, 0.332, 0.332, 0.331, 0.331, 0.33, 0.329, 0.328, 0.328, 0.327, 0.326, 0.325, 0.324,
    0.324, 0.324, 0.323, 0.322, 0.321, 0.32, 0.318, 0.316, 0.315, 0.315, 0.314, 0.314],
    [0.065, 0.068, 0.07, 0.072, 0.073, 0.073, 0.074, 0.074, 0.074, 0.073, 0.073, 0.073, 0.073,
    0.073, 0.074, 0.075, 0.077, 0.08, 0.085, 0.094, 0.109, 0.126, 0.148, 0.172, 0.198, 0.221, 0.241,
    0.26, 0.278, 0.302, 0.339, 0.37, 0.392, 0.399, 0.4, 0.393, 0.38, 0.365, 0.349, 0.332, 0.315,
    0.299, 0.285, 0.272, 0.264, 0.257, 0.252, 0.247, 0.241, 0.235, 0.229, 0.224, 0.22, 0.217, 0.216,
    0.216, 0.219, 0.224, 0.23, 0.238, 0.251, 0.269, 0.288, 0.312, 0.34, 0.366, 0.39, 0.412, 0.431,
    0.447, 0.46, 0.472, 0.481, 0.488, 0.493, 0.497, 0.5, 0.502, 0.505, 0.51, 0.516],
    [0.074, 0.083, 0.093, 0.105, 0.116, 0.121, 0.124, 0.126, 0.128, 0.131, 0.135, 0.139, 0.144,
    0.151, 0.161, 0.172, 0.186, 0.205, 0.229, 0.254, 0.281, 0.308, 0.332, 0.352, 0.37, 0.383, 0.39,
    0.394, 0.395, 0.392, 0.385, 0.377, 0.367, 0.354, 0.341, 0.327, 0.312, 0.296, 0.28, 0.263, 0.247,
    0.229, 0.214, 0.198, 0.185, 0.175, 0.169, 0.164, 0.16, 0.156, 0.154, 0.152, 0.151, 0.149, 0.148,
    0.148, 0.148, 0.149, 0.151, 0.154, 0.158, 0.162, 0.165, 0.168, 0.17, 0.171, 0.17, 0.168, 0.166,
    0.164, 0.164, 0.165, 0.168, 0.172, 0.177, 0.181, 0.185, 0.189, 0.192, 0.194, 0.197],
    [0.295, 0.306, 0.31, 0.312, 0.313, 0.315, 0.319, 0.322, 0.326, 0.33, 0.334, 0.339, 0.346, 0.352,
    0.36, 0.369, 0.381, 0.394, 0.403, 0.41, 0.415, 0.418, 0.419, 0.417, 0.413, 0.409, 0.403, 0.396,
    0.389, 0.381, 0.372, 0.363, 0.353, 0.342, 0.331, 0.32, 0.308, 0.296, 0.284, 0.271, 0.26, 0.247,
    0.232, 0.22, 0.21, 0.2, 0.194, 0.189, 0.185, 0.183, 0.18, 0.177, 0.176, 0.175, 0.175, 0.175,
    0.175, 0.177, 0.18, 0.183, 0.186, 0.189, 0.192, 0.195, 0.199, 0.2, 0.199, 0.198, 0.196, 0.195,
    0.195, 0.196, 0.197, 0.2, 0.203, 0.205, 0.208, 0.212, 0.215, 0.217, 0.219],
    [0.151, 0.203, 0.265, 0.339, 0.41, 0.464, 0.492, 0.508, 0.517, 0.524, 0.531, 0.538, 0.544,
    0.551, 0.556, 0.556, 0.554, 0.549, 0.541, 0.531, 0.519, 0.504, 0.488, 0.469, 0.45, 0.431, 0.414,
    0.395, 0.377, 0.358, 0.341, 0.325, 0.309, 0.293, 0.279, 0.265, 0.253, 0.241, 0.234, 0.227,
    0.225, 0.222, 0.221, 0.22, 0.22, 0.22, 0.22, 0.22, 0.223, 0.227, 0.233, 0.239, 0.244, 0.251,
    0.258, 0.263, 0.268, 0.273, 0.278, 0.281, 0.283, 0.286, 0.291, 0.296, 0.302, 0.313, 0.325,
    0.338, 0.351, 0.364, 0.376, 0.389, 0.401, 0.413, 0.425, 0.436, 0.447, 0.458, 0.469, 0.477,
    0.485],
    [0.378, 0.459, 0.524, 0.546, 0.551, 0.555, 0.559, 0.56, 0.561, 0.558, 0.556, 0.551, 0.544,
    0.535, 0.522, 0.506, 0.488, 0.469, 0.448, 0.429, 0.408, 0.385, 0.363, 0.341, 0.324, 0.311,
    0.301, 0.291, 0.283, 0.273, 0.265, 0.26, 0.257, 0.257, 0.259, 0.26, 0.26, 0.258, 0.256, 0.254,
    0.254, 0.259, 0.27, 0.284, 0.302, 0.324, 0.344, 0.362, 0.377, 0.389, 0.4, 0.41, 0.42, 0.429,
    0.438, 0.445, 0.452, 0.457, 0.462, 0.466, 0.468, 0.47, 0.473, 0.477, 0.483, 0.489, 0.496, 0.503,
    0.511, 0.518, 0.525, 0.532, 0.539, 0.546, 0.553, 0.559, 0.565, 0.57, 0.575, 0.578, 0.581],
    [0.104, 0.129, 0.17, 0.24, 0.319, 0.416, 0.462, 0.482, 0.49, 0.488, 0.482, 0.473, 0.462, 0.45,
    0.439, 0.426, 0.413, 0.397, 0.382, 0.366, 0.352, 0.337, 0.325, 0.31, 0.299, 0.289, 0.283, 0.276,
    0.27, 0.262, 0.256, 0.251, 0.25, 0.251, 0.254, 0.258, 0.264, 0.269, 0.272, 0.274, 0.278, 0.284,
    0.295, 0.316, 0.348, 0.384, 0.434, 0.482, 0.528, 0.568, 0.604, 0.629, 0.648, 0.663, 0.676,
    0.685, 0.693, 0.7, 0.705, 0.709, 0.712, 0.715, 0.717, 0.719, 0.721, 0.72, 0.719, 0.722, 0.725,
    0.727, 0.729, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73],
    [0.066, 0.062, 0.058, 0.055, 0.052, 0.052, 0.051, 0.05, 0.05, 0.049, 0.048, 0.047, 0.046, 0.044,
    0.042, 0.041, 0.038, 0.035, 0.033, 0.031, 0.03, 0.029, 0.028, 0.028, 0.028, 0.029, 0.03, 0.03,
    0.031, 0.031, 0.032, 0.032, 0.033, 0.034, 0.035, 0.037, 0.041, 0.044, 0.048, 0.052, 0.06, 0.076,
    0.102, 0.136, 0.19, 0.256, 0.336, 0.418, 0.505, 0.581, 0.641, 0.682, 0.717, 0.74, 0.758, 0.77,
    0.781, 0.79, 0.797, 0.803, 0.809, 0.814, 0.819, 0.824, 0.828, 0.83, 0.831, 0.833, 0.835, 0.836,
    0.836, 0.837, 0.838, 0.839, 0.839, 0.839, 0.839, 0.839, 0.839, 0.839, 0.839],
    [0.05, 0.054, 0.059, 0.063, 0.066, 0.067, 0.068, 0.069, 0.069, 0.07, 0.072, 0.073, 0.076, 0.078,
    0.083, 0.088, 0.095, 0.103, 0.113, 0.125, 0.142, 0.162, 0.189, 0.219, 0.262, 0.305, 0.365,
    0.416, 0.465, 0.509, 0.546, 0.581, 0.61, 0.634, 0.653, 0.666, 0.678, 0.687, 0.693, 0.698, 0.701,
    0.704, 0.705, 0.705, 0.706, 0.707, 0.707, 0.707, 0.708, 0.708, 0.71, 0.711, 0.712, 0.714, 0.716,
    0.718, 0.72, 0.722, 0.725, 0.729, 0.731, 0.735, 0.739, 0.742, 0.746, 0.748, 0.749, 0.751, 0.753,
    0.754, 0.755, 0.755, 0.755, 0.755, 0.756, 0.757, 0.758, 0.759, 0.759, 0.759, 0.759],
    [0.111, 0.121, 0.127, 0.129, 0.127, 0.121, 0.116, 0.112, 0.108, 0.105, 0.104, 0.104, 0.105,
    0.106, 0.11, 0.115, 0.123, 0.134, 0.148, 0.167, 0.192, 0.219, 0.252, 0.291, 0.325, 0.347, 0.356,
    0.353, 0.346, 0.333, 0.314, 0.294, 0.271, 0.248, 0.227, 0.206, 0.188, 0.17, 0.153, 0.138, 0.125,
    0.114, 0.106, 0.1, 0.096, 0.092, 0.09, 0.087, 0.085, 0.082, 0.08, 0.079, 0.078, 0.078, 0.078,
    0.078, 0.081, 0.083, 0.088, 0.093, 0.102, 0.112, 0.125, 0.141, 0.161, 0.182, 0.203, 0.223,
    0.242, 0.257, 0.27, 0.282, 0.292, 0.302, 0.31, 0.314, 0.317, 0.323, 0.33, 0.334, 0.338],
    [0.12, 0.103, 0.09, 0.082, 0.076, 0.068, 0.064, 0.065, 0.075, 0.093, 0.123, 0.16, 0.207, 0.256,
    0.3, 0.331, 0.346, 0.347, 0.341, 0.328, 0.307, 0.282, 0.257, 0.23, 0.204, 0.178, 0.154, 0.129,
    0.109, 0.09, 0.075, 0.062, 0.051, 0.041, 0.035, 0.029, 0.025, 0.022, 0.019, 0.017, 0.017, 0.017,
    0.016, 0.016, 0.016, 0.016, 0.016, 0.016, 0.016, 0.016, 0.018, 0.018, 0.018, 0.018, 0.019, 0.02,
    0.023, 0.024, 0.026, 0.03, 0.035, 0.043, 0.056, 0.074, 0.097, 0.128, 0.166, 0.21, 0.257, 0.305,
    0.354, 0.401, 0.446, 0.485, 0.52, 0.551, 0.577, 0.599, 0.618, 0.633, 0.645],
    [0.104, 0.127, 0.161, 0.211, 0.264, 0.313, 0.341, 0.352, 0.359, 0.361, 0.364, 0.365, 0.367,
    0.369, 0.372, 0.374, 0.376, 0.379, 0.384, 0.389, 0.397, 0.405, 0.416, 0.429, 0.443, 0.454,
    0.461, 0.466, 0.469, 0.471, 0.474, 0.476, 0.483, 0.49, 0.506, 0.526, 0.553, 0.582, 0.618, 0.651,
    0.68, 0.701, 0.717, 0.729, 0.736, 0.742, 0.745, 0.747, 0.748, 0.748, 0.748, 0.748, 0.748, 0.748,
    0.748, 0.748, 0.747, 0.747, 0.747, 0.747, 0.747, 0.747, 0.747, 0.746, 0.746, 0.746, 0.745,
    0.744, 0.743, 0.744, 0.745, 0.748, 0.75, 0.75, 0.749, 0.748, 0.748, 0.747, 0.747, 0.747, 0.747],
    [0.036, 0.036, 0.037, 0.038, 0.039, 0.039, 0.04, 0.041, 0.042, 0.042, 0.043, 0.044, 0.044,
    0.045, 0.045, 0.046, 0.047, 0.048, 0.05, 0.052, 0.055, 0.057, 0.062, 0.067, 0.075, 0.083, 0.092,
    0.1, 0.108, 0.121, 0.133, 0.142, 0.15, 0.154, 0.155, 0.152, 0.147, 0.14, 0.133, 0.125, 0.118,
    0.112, 0.106, 0.101, 0.098, 0.095, 0.093, 0.09, 0.089, 0.087, 0.086, 0.085, 0.084, 0.084, 0.084,
    0.084, 0.085, 0.087, 0.092, 0.096, 0.102, 0.11, 0.123, 0.137, 0.152, 0.169, 0.188, 0.207, 0.226,
    0.243, 0.26, 0.277, 0.294, 0.31, 0.325, 0.339, 0.353, 0.366, 0.379, 0.39, 0.399],

]));