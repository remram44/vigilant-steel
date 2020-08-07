use log::error;
use vecmath::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum BufType {
    STATIC = 0,
    DYNAMIC = 1,
    STREAM = 2,
}

#[wasm_bindgen]
extern "C" {
    fn set_buffer(
        id: f64, vertex_array: &[f32],
        color_array: &[f32],
        mode: i32,
    );
}

#[derive(Clone, Default)]
pub struct VertexVecs {
    vertexes: Vec<f32>,
    colors: Vec<f32>,
}

pub trait VertexArrays {
    fn arrays(&mut self) -> (&mut Vec<f32>, &mut Vec<f32>);
    fn vertexes(&self) -> &[f32];
    fn colors(&self) -> &[f32];
    fn matrix(&self) -> &Matrix2x3<f32>;

    /// Turn the arrays into GL buffers
    fn store(self, id: f64, mode: BufType);

    fn is_empty(&self) -> bool {
        self.vertexes().is_empty()
    }

    fn translate<'a>(
        &'a mut self,
        x: f32, y: f32,
    ) -> VertexArraysTransformed<'a> {
        let matrix = self.matrix().clone();
        let (vertexes, colors) = self.arrays();
        VertexArraysTransformed {
            vertexes,
            colors,
            matrix: row_mat2x3_mul(
                matrix,
                [
                    [1.0, 0.0, x],
                    [0.0, 1.0, y],
                ],
            ),
        }
    }

    fn rotate<'a>(
        &'a mut self,
        angle: f32,
    ) -> VertexArraysTransformed<'a> {
        let matrix = self.matrix().clone();
        let (vertexes, colors) = self.arrays();
        let (s, c) = angle.sin_cos();
        VertexArraysTransformed {
            vertexes,
            colors,
            matrix: row_mat2x3_mul(
                matrix,
                [
                    [c, -s, 0.0],
                    [s, c, 0.0],
                ],
            ),
        }
    }

    fn transform(&self, point: [f32; 2]) -> [f32; 2] {
        row_mat2x3_transform_pos2(self.matrix().clone(), point)
    }

    fn transform_vec(&self, vec: [f32; 2]) -> [f32; 2] {
        row_mat2x3_transform_vec2(self.matrix().clone(), vec)
    }

    /// Generates a line
    fn line(
        &mut self,
        pos1: [f32; 2], pos2: [f32; 2],
        width: f32, color: [f32; 4],
    ) {
        let pos1 = self.transform(pos1);
        let pos2 = self.transform(pos2);
        let (vertexes, colors) = self.arrays();
        let len = {
            let dx = pos2[0] - pos1[0];
            let dy = pos2[1] - pos1[1];
            (dx * dx + dy * dy).sqrt()
        };
        let ortho = [
            (pos2[1] - pos1[1]) / len,
            -(pos2[0] - pos1[0]) / len,
        ];
        let p1x = pos1[0] + 0.5 * width * ortho[0];
        let p1y = pos1[1] + 0.5 * width * ortho[1];
        let p2x = pos1[0] - 0.5 * width * ortho[0];
        let p2y = pos1[1] - 0.5 * width * ortho[1];
        let p3x = pos2[0] + 0.5 * width * ortho[0];
        let p3y = pos2[1] + 0.5 * width * ortho[1];
        let p4x = pos2[0] - 0.5 * width * ortho[0];
        let p4y = pos2[1] - 0.5 * width * ortho[1];
        vertexes.extend_from_slice(&[
            p1x, p1y, p2x, p2y, p3x, p3y,
            p3x, p3y, p2x, p2y, p4x, p4y,
        ]);
        for _ in 0..6 {
            colors.extend_from_slice(&color);
        }
    }

    /// Generates a hollow rectangle
    fn hollow_rect(
        &mut self,
        corner1: [f32; 2], corner2: [f32; 2],
        width: f32, color: [f32; 4],
    ) {
        self.line(
            [corner1[0], corner1[1]], [corner2[0], corner1[1]],
            width, color,
        );
        self.line(
            [corner2[0], corner1[1]], [corner2[0], corner2[1]],
            width, color,
        );
        self.line(
            [corner2[0], corner2[1]], [corner1[0], corner2[1]],
            width, color,
        );
        self.line(
            [corner1[0], corner2[1]], [corner1[0], corner1[1]],
            width, color,
        );
    }

    /// Generate a looped polyline
    fn polygon(
        &mut self,
        points: &[[f32; 2]],
        width: f32, color: [f32; 4],
    ) {
        for i in 0..points.len() + 1 {
            self.line(
                points[i % points.len()],
                points[(i + 1) % points.len()],
                width, color,
            );
        }
    }

    /// Generates a filled rectangle
    fn filled_rect(
        &mut self,
        corner1: [f32; 2], corner2: [f32; 2],
        color: [f32; 4],
    ) {
        let p1 = self.transform([corner1[0], corner1[1]]);
        let p2 = self.transform([corner1[0], corner2[1]]);
        let p3 = self.transform([corner2[0], corner1[1]]);
        let p4 = self.transform([corner2[0], corner2[1]]);
        let (vertexes, colors) = self.arrays();
        vertexes.extend_from_slice(&[
            p1[0], p1[1],
            p2[0], p2[1],
            p4[0], p4[1],
            p4[0], p4[1],
            p3[0], p3[1],
            p1[0], p1[1],
        ]);
        for _ in 0..6 {
            colors.extend_from_slice(&color);
        }
    }

    /// Generates a single filled triangle
    fn filled_triangle(
        &mut self,
        points: &[[f32; 2]],
        color: [f32; 4],
    ) {
        if points.len() != 3 {
            error!("filled_triangle() with {} points", points.len());
        }
        let p1 = self.transform(points[0]);
        let p2 = self.transform(points[1]);
        let p3 = self.transform(points[2]);
        let (vertexes, colors) = self.arrays();
        vertexes.extend_from_slice(&[
            p1[0], p1[1],
            p2[0], p2[1],
            p3[0], p3[1],
        ]);
        for _ in 0..3 {
            colors.extend_from_slice(&color);
        }
    }

    /// Generates a filled convex polygon
    fn filled_convex_polygon(
        &mut self,
        points: &[[f32; 2]],
        color: [f32; 4],
    ) {
        for i in 1..points.len() - 1 {
            self.filled_triangle(
                &[points[0], points[i], points[i + 1]],
                color,
            );
        }
    }
}

impl VertexArrays for VertexVecs {
    fn arrays(&mut self) -> (&mut Vec<f32>, &mut Vec<f32>) {
        (&mut self.vertexes, &mut self.colors)
    }

    fn vertexes(&self) -> &[f32] {
        &self.vertexes
    }

    fn colors(&self) -> &[f32] {
        &self.colors
    }

    fn matrix(&self) -> &Matrix2x3<f32> {
        &[
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
    }

    fn store(self, id: f64, mode: BufType) {
        set_buffer(id, self.vertexes(), self.colors(), mode as i32)
    }

    fn transform(&self, point: [f32; 2]) -> [f32; 2] {
        point
    }

    fn transform_vec(&self, vec: [f32; 2]) -> [f32; 2] {
        vec
    }
}

pub struct VertexArraysTransformed<'a> {
    vertexes: &'a mut Vec<f32>,
    colors: &'a mut Vec<f32>,
    matrix: Matrix2x3<f32>,
}

impl<'a> VertexArrays for VertexArraysTransformed<'a> {
    fn arrays(&mut self) -> (&mut Vec<f32>, &mut Vec<f32>) {
        (self.vertexes, self.colors)
    }

    fn vertexes(&self) -> &[f32] {
        self.vertexes
    }

    fn colors(&self) -> &[f32] {
        self.colors
    }

    fn matrix(&self) -> &Matrix2x3<f32> {
        &self.matrix
    }

    fn store(self, id: f64, mode: BufType) {
        set_buffer(id, self.vertexes(), self.colors(), mode as i32)
    }
}
