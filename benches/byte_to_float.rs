//! Evaluating converting from u8 [0, 255] to f32 [0, 1] using either naive conversion or a lookup
//! table.
//!
//! I found that the naive version actually optimizes better because it can be vectorized while
//! the lookup apparently can't. The difference is even more striking with AVX2 which provides even
//! wider SIMD lanes for the conversion.
//!
//! The difference is not explained by bounds checking because the lookup doesn't appear to
//! emit any: https://godbolt.org/z/sutBRr
//! In fact, `.get_unchecked()` seems to make it perform *worse*. Try it.

#[macro_use]
extern crate criterion;

use criterion::{Bencher, Criterion, ParameterizedBenchmark, Throughput};

static LOOKUP: [f32; 256] = [0.0, 0.003921569, 0.007843138, 0.011764706, 0.015686275, 0.019607844, 0.023529412, 0.02745098, 0.03137255, 0.03529412, 0.039215688, 0.043137256, 0.047058824, 0.050980393, 0.05490196, 0.05882353, 0.0627451, 0.06666667, 0.07058824, 0.07450981, 0.078431375, 0.08235294, 0.08627451, 0.09019608, 0.09411765, 0.09803922, 0.101960786, 0.105882354, 0.10980392, 0.11372549, 0.11764706, 0.12156863, 0.1254902, 0.12941177, 0.13333334, 0.13725491, 0.14117648, 0.14509805, 0.14901961, 0.15294118, 0.15686275, 0.16078432, 0.16470589, 0.16862746, 0.17254902, 0.1764706, 0.18039216, 0.18431373, 0.1882353, 0.19215687, 0.19607843, 0.2, 0.20392157, 0.20784314, 0.21176471, 0.21568628, 0.21960784, 0.22352941, 0.22745098, 0.23137255, 0.23529412, 0.23921569, 0.24313726, 0.24705882, 0.2509804, 0.25490198, 0.25882354, 0.2627451, 0.26666668, 0.27058825, 0.27450982, 0.2784314, 0.28235295, 0.28627452, 0.2901961, 0.29411766, 0.29803923, 0.3019608, 0.30588236, 0.30980393, 0.3137255, 0.31764707, 0.32156864, 0.3254902, 0.32941177, 0.33333334, 0.3372549, 0.34117648, 0.34509805, 0.34901962, 0.3529412, 0.35686275, 0.36078432, 0.3647059, 0.36862746, 0.37254903, 0.3764706, 0.38039216, 0.38431373, 0.3882353, 0.39215687, 0.39607844, 0.4, 0.40392157, 0.40784314, 0.4117647, 0.41568628, 0.41960785, 0.42352942, 0.42745098, 0.43137255, 0.43529412, 0.4392157, 0.44313726, 0.44705883, 0.4509804, 0.45490196, 0.45882353, 0.4627451, 0.46666667, 0.47058824, 0.4745098, 0.47843137, 0.48235294, 0.4862745, 0.49019608, 0.49411765, 0.49803922, 0.5019608, 0.5058824, 0.50980395, 0.5137255, 0.5176471, 0.52156866, 0.5254902, 0.5294118, 0.53333336, 0.5372549, 0.5411765, 0.54509807, 0.54901963, 0.5529412, 0.5568628, 0.56078434, 0.5647059, 0.5686275, 0.57254905, 0.5764706, 0.5803922, 0.58431375, 0.5882353, 0.5921569, 0.59607846, 0.6, 0.6039216, 0.60784316, 0.6117647, 0.6156863, 0.61960787, 0.62352943, 0.627451, 0.6313726, 0.63529414, 0.6392157, 0.6431373, 0.64705884, 0.6509804, 0.654902, 0.65882355, 0.6627451, 0.6666667, 0.67058825, 0.6745098, 0.6784314, 0.68235296, 0.6862745, 0.6901961, 0.69411767, 0.69803923, 0.7019608, 0.7058824, 0.70980394, 0.7137255, 0.7176471, 0.72156864, 0.7254902, 0.7294118, 0.73333335, 0.7372549, 0.7411765, 0.74509805, 0.7490196, 0.7529412, 0.75686276, 0.7607843, 0.7647059, 0.76862746, 0.77254903, 0.7764706, 0.78039217, 0.78431374, 0.7882353, 0.7921569, 0.79607844, 0.8, 0.8039216, 0.80784315, 0.8117647, 0.8156863, 0.81960785, 0.8235294, 0.827451, 0.83137256, 0.8352941, 0.8392157, 0.84313726, 0.84705883, 0.8509804, 0.85490197, 0.85882354, 0.8627451, 0.8666667, 0.87058824, 0.8745098, 0.8784314, 0.88235295, 0.8862745, 0.8901961, 0.89411765, 0.8980392, 0.9019608, 0.90588236, 0.9098039, 0.9137255, 0.91764706, 0.92156863, 0.9254902, 0.92941177, 0.93333334, 0.9372549, 0.9411765, 0.94509804, 0.9490196, 0.9529412, 0.95686275, 0.9607843, 0.9647059, 0.96862745, 0.972549, 0.9764706, 0.98039216, 0.9843137, 0.9882353, 0.99215686, 0.99607843, 1.0];

fn bench_functions(c: &mut Criterion) {
    let ref sizes = [64usize, 128, 256, 384, 512, 768, 1024];

    let bench = ParameterizedBenchmark::new(
        "lookup",
        move |b: &mut Bencher, &&size| {
            let vals: Vec<u8> = (0 ..= 255).cycle().take(size).collect();

            b.iter_with_setup(
                || Vec::with_capacity(size),
                move |mut out: Vec<f32>| out.extend(vals.iter().map(|&x| LOOKUP[x as usize]))
            );
        },
        sizes
    ).with_function(
        "naive",
        |b: &mut Bencher, &&size| {
            let vals: Vec<u8> = (0 ..= 255).cycle().take(size).collect();

            b.iter_with_setup(
                || Vec::with_capacity(size),
                |mut out| out.extend(vals.iter().map(|&x| x as f32 / 255.))
            );
        }
    ).throughput(|&&size| Throughput::Bytes(size as _));

    c.bench("byte to float conversion", bench);
}


criterion_group!(benches, bench_functions);
criterion_main!(benches);
