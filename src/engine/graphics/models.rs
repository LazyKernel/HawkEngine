use std::{fs::File};

use nalgebra::Vector3;

use crate::data_structures::graphics::Vertex;


// Creates a height field matrix from a given heightmap image
// Uses simple interpolation when the size of the image doesn't match
// the height field size 1-to-1
pub fn create_height_field(path: &String, field_width: u32, field_height: u32) -> Vec<Vec<f32>> {
    // TODO: clean up unwraps

    let image = File::open(path).unwrap();
    let decoder = png::Decoder::new(image);
    let mut reader = decoder.read_info().unwrap();

    let (w, h) = reader.info().size();
    let (scalew, scaleh) = ((w / field_width) as f64, (h / field_height) as f64);

    let mut pixels = vec![0; reader.info().raw_bytes()];
    reader.next_frame(&mut pixels).unwrap();

    let (fw, fh): (usize, usize) = (field_width.try_into().unwrap(), field_height.try_into().unwrap());

    let mut height_field = vec![vec![0.0_f32; fw]; fh];

    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    while y.floor() < fh as f64 {
        let yf: usize = (y.floor() as u64).try_into().unwrap();
        while x.floor() < fw as f64 {
            let xf: usize = (x.floor() as u64).try_into().unwrap();
            // row-wise packed, assuming rgba, taking the red channel
            // TODO: support different formats?
            let val = pixels[yf * fw + xf * 4];
            let scaled_val = val as f32 / 255.0;
            height_field[yf][xf] = scaled_val;
            x += scalew;
        }
        y += scaleh;
    }

    return height_field;
}

fn get_smooth_normal(x: usize, y: usize, h: usize, w: usize, hf: &Vec<Vec<f32>>) -> Vector3<f32> {
    let mut normal = Vector3::<f32>::zeros();

    // checking if each corner is in range and calculating the triangle norms for the 2 triangles between origin and the corner
    if x > 0 && y > 0 {
        let v1 = Vector3::new((x - 1) as f32, hf[y-1][x-1], (y - 1) as f32);
        let v2 = Vector3::new((x - 1) as f32, hf[y][x-1], y as f32);
        let v3 = Vector3::new(x as f32,  hf[y-1][x], (y - 1) as f32);
        let vo = Vector3::new(x as f32, hf[y][x], y as f32);

        let p1 = (v1 - vo).cross(&(v2 - vo));
        let p2 = (v1 - vo).cross(&(v3 - vo));

        normal += p1 + p2;
    }

    if x + 1 < w && y > 0 {
        let v1 = Vector3::new((x + 1) as f32, hf[y-1][x+1], (y - 1) as f32);
        let v2 = Vector3::new((x + 1) as f32, hf[y][x+1], y as f32);
        let v3 = Vector3::new(x as f32, hf[y-1][x], (y - 1) as f32);
        let vo = Vector3::new(x as f32, hf[y][x], y as f32);

        let p1 = (v1 - vo).cross(&(v2 - vo));
        let p2 = (v1 - vo).cross(&(v3 - vo));

        normal += p1 + p2;
    }

    if x > 0 && y + 1 < h {
        let v1 = Vector3::new((x - 1) as f32, hf[y+1][x-1], (y + 1) as f32);
        let v2 = Vector3::new((x - 1) as f32, hf[y][x-1], y as f32);
        let v3 = Vector3::new(x as f32, hf[y+1][x], (y + 1) as f32);
        let vo = Vector3::new(x as f32, hf[y][x], y as f32);

        let p1 = (v1 - vo).cross(&(v2 - vo));
        let p2 = (v1 - vo).cross(&(v3 - vo));

        normal += p1 + p2;
    }

    if x + 1 < w && y + 1 < h {
        let v1 = Vector3::new((x + 1) as f32, hf[y+1][x+1], (y + 1) as f32);
        let v2 = Vector3::new((x + 1) as f32, hf[y][x+1], y as f32);
        let v3 = Vector3::new(x as f32, hf[y+1][x], (y + 1) as f32);
        let vo = Vector3::new(x as f32, hf[y][x], y as f32);

        let p1 = (v1 - vo).cross(&(v2 - vo));
        let p2 = (v1 - vo).cross(&(v3 - vo));

        normal += p1 + p2;
    }

    return normal.normalize();
}

// Creates terrain vertices from a height field
pub fn create_terrain_vertices(height_field: Vec<Vec<f32>>) -> (Vec<Vertex>, Vec<usize>) {
    let (h, w) = (height_field.len(), height_field[0].len());
    let mut verts = Vec::<Vertex>::with_capacity(h * w);
    let mut indices = Vec::<usize>::with_capacity(h * w);

    for y in 0..h {
        for x in 0..w {
            let z = height_field[y][x];
            // y is up in our coordinate system but were thinking of the height field as a texture (x,y plane)
            let vert = Vertex {
                position: [x as f32, z, y as f32],
                normal: get_smooth_normal(x, y, h, w, &height_field).into(),
                color: [1.0, 1.0, 1.0],
                tex_coord: [x as f32 / w as f32, y as f32 / h as f32]
            };
            verts.push(vert);

            // pushing the 2 triangles comprising the quad where this vertex is the top right, if possible
            if x > 0 && y > 0 {
                let idx_vc = (y-1) * h + (x-1);
                let idx_v2 = (y) * h + (x-1);
                let idx_v3 = (y-1) * h + (x);
                let idx_vo = y * h + x;

                indices.extend([
                    idx_vc, idx_v2, idx_vo,
                    idx_vo, idx_v3, idx_vc
                ]);
            }
        }
    }

    println!("number of indices: {}", indices.len());
    return (verts, indices);
}