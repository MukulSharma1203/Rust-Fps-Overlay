use std::env;
use std::fs;
use std::path::PathBuf;

fn draw_rust_icon_ico(path: &PathBuf) {
    let size: usize = 64;
    let mut pixels = vec![[0u8; 4]; size * size]; // BGRA
    let cx = 31.5f32;
    let cy = 31.5f32;

    // Helper to check if a point lies inside a bold letter 'R'
    let in_letter_r = |x: f32, y: f32| -> bool {
        let rx = x - cx;
        let ry = y - cy;
        // Left vertical stem
        if (-9.5..=-4.2).contains(&rx) && (-11.5..=11.5).contains(&ry) {
            return true;
        }
        // Top & middle horizontal bars of the bowl
        if (-9.5..=3.5).contains(&rx) && ((-11.5..=-6.5).contains(&ry) || (-1.5..=3.0).contains(&ry)) {
            return true;
        }
        // Rounded right bowl of 'R'
        let bowl_cx = 3.0f32;
        let bowl_cy = -4.25f32;
        let bd = ((rx - bowl_cx).powi(2) + (ry - bowl_cy).powi(2)).sqrt();
        if rx >= 2.0 && (2.2..=7.3).contains(&bd) {
            return true;
        }
        // Diagonal right leg
        if ry >= 2.0 && ry <= 11.5 {
            let leg_center = 1.5 + (ry - 2.0) * 0.72;
            if (rx - leg_center).abs() <= 2.9 {
                return true;
            }
        }
        false
    };

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);

            // 16-tooth Rust gear profile
            let tooth_wave = (angle * 16.0).cos();
            let outer_r = if tooth_wave > 0.15 { 30.2 } else { 25.5 };

            let color = if dist <= outer_r {
                if dist >= 19.5 {
                    // Rust burnt orange gear ring & teeth (#F74C00 -> BGRA)
                    [0x00, 0x4C, 0xF7, 0xFF]
                } else if in_letter_r(x as f32, y as f32) {
                    // Crisp white 'R'
                    [0xFF, 0xFF, 0xFF, 0xFF]
                } else {
                    // Dark charcoal center disc (#1B1B1E)
                    [0x1E, 0x1B, 0x1B, 0xFF]
                }
            } else {
                [0, 0, 0, 0]
            };

            pixels[y * size + x] = color;
        }
    }

    let xor_bytes = size * size * 4;
    let and_bytes = size * (size / 8); // 64 * 8 = 512 bytes
    let img_bytes = 40 + xor_bytes + and_bytes;

    let mut ico = Vec::with_capacity(22 + img_bytes);
    // ICONDIR (6 bytes)
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    // ICONDIRENTRY (16 bytes)
    ico.push(size as u8);
    ico.push(size as u8);
    ico.push(0);
    ico.push(0);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(img_bytes as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    // BITMAPINFOHEADER (40 bytes)
    ico.extend_from_slice(&40u32.to_le_bytes());
    ico.extend_from_slice(&(size as i32).to_le_bytes());
    ico.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&0u32.to_le_bytes());
    ico.extend_from_slice(&((xor_bytes + and_bytes) as u32).to_le_bytes());
    ico.extend_from_slice(&[0u8; 16]);

    // Bottom-up BGRA pixel data
    for y in (0..size).rev() {
        for x in 0..size {
            ico.extend_from_slice(&pixels[y * size + x]);
        }
    }
    // AND mask (all zeros; alpha channel handles transparency)
    ico.extend_from_slice(&vec![0u8; and_bytes]);

    fs::write(path, ico).expect("Failed to write rust_logo.ico");
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let icon_path = out_dir.join("rust_logo.ico");
    draw_rust_icon_ico(&icon_path);

    let mut res = winresource::WindowsResource::new();
    res.set_icon_with_id(icon_path.to_str().unwrap(), "1");
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#,
    );
    res.compile().expect("Failed to compile Windows resources");
}