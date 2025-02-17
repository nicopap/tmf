use crate::tmf::DecodedSegment;
use crate::{
    FloatType, IndexType, TMFExportError, TMFMesh, TMFPrecisionInfo, Vector3, MIN_TMF_MAJOR,
    MIN_TMF_MINOR, TMF_MAJOR, TMF_MINOR,
};
pub(crate) struct EncodeInfo {
    shortest_edge: FloatType,
}
impl Default for EncodeInfo {
    fn default() -> Self {
        Self { shortest_edge: 0.1 }
    }
}
impl EncodeInfo {
    pub(crate) fn shortest_edge(&self) -> FloatType {
        self.shortest_edge
    }
}
fn calc_shortest_edge(
    vertex_triangles: Option<&[IndexType]>,
    vertices: Option<&[Vector3]>,
) -> FloatType {
    let shortest_edge = match vertex_triangles {
        Some(vertex_triangles) => {
            if vertex_triangles.is_empty() {
                //TODO: handle 0 faced mesh as mesh with no faces!
                return 0.1;
            }
            use crate::utilis::distance;
            let vertices = match vertices {
                Some(vertices) => vertices,
                None => return 0.1,
            };
            let mut shortest_edge = FloatType::INFINITY;
            for i in 0..(vertex_triangles.len() / 3) {
                let d1 = distance(
                    vertices[vertex_triangles[i * 3] as usize],
                    vertices[vertex_triangles[i * 3 + 1] as usize],
                );
                let d2 = distance(
                    vertices[vertex_triangles[i * 3 + 1] as usize],
                    vertices[vertex_triangles[i * 3 + 2] as usize],
                );
                let d3 = distance(
                    vertices[vertex_triangles[i * 3 + 2] as usize],
                    vertices[vertex_triangles[i * 3] as usize],
                );
                shortest_edge = shortest_edge.min(d1.min(d2.min(d3)));
            }
            shortest_edge
        }
        // TODO: Calculate distance between closest points for point cloud
        None => 0.1,
    };
    assert!(
        shortest_edge.is_finite(),
        "Shortest edge should be finite but is '{shortest_edge}'!"
    );
    shortest_edge
}

pub(crate) fn write_mesh_name<W: std::io::Write>(w: &mut W, s: &str) -> std::io::Result<()> {
    let bytes = s.as_bytes();
    w.write_all(&(bytes.len() as u16).to_le_bytes())?;
    w.write_all(bytes)
}
async fn write_mesh<W: std::io::Write>(
    mesh: &TMFMesh,
    name: &str,
    target: &mut W,
    p_info: &TMFPrecisionInfo,
) -> Result<(), TMFExportError> {
    write_mesh_name(target, name)?;
    let ei = EncodeInfo {
        shortest_edge: calc_shortest_edge(mesh.get_vertex_triangles(), mesh.get_vertices()),
    };
    let tmf_segs = MeshSegIter::tmf_segs(mesh);
    let mut new_segs = Vec::with_capacity(32);
    for seg in tmf_segs {
        let c_segs = seg.optimize().await;
        for c_seg in c_segs {
            new_segs.push(c_seg);
        }
    }
    //println!("Optimized segs:{}", new_segs.len());
    let tmf_segs = new_segs;
    let mut encoded = Vec::with_capacity(tmf_segs.len());
    for seg in tmf_segs {
        encoded.push(seg.encode(p_info, &ei));
    }
    let encoded = futures::future::join_all(encoded).await;
    target.write_all(&(encoded.len() as u16).to_le_bytes())?;
    for seg in encoded {
        seg?.write(target)?;
    }
    Ok(())
}
pub(crate) async fn write_tmf<W: std::io::Write, S: std::borrow::Borrow<str>>(
    meshes_names: &[(TMFMesh, S)],
    target: &mut W,
    p_info: &TMFPrecisionInfo,
) -> Result<(), TMFExportError> {
    let mesh_count = meshes_names.len();
    write_tmf_header(target, mesh_count as u32)?;
    for (mesh, name) in meshes_names {
        write_mesh(mesh, name.borrow(), target, p_info).await?;
    }
    Ok(())
}
pub(crate) fn write_tmf_header<W: std::io::Write>(
    w: &mut W,
    mesh_count: u32,
) -> Result<(), TMFExportError> {
    w.write_all(b"TMF")?;
    w.write_all(&TMF_MAJOR.to_le_bytes())?;
    w.write_all(&(TMF_MINOR).to_le_bytes())?;
    w.write_all(&MIN_TMF_MAJOR.to_le_bytes())?;
    w.write_all(&(MIN_TMF_MINOR).to_le_bytes())?;
    Ok(w.write_all(&mesh_count.to_le_bytes())?)
}
#[cfg(test)]
fn init_test_env() {
    std::fs::create_dir_all("target/test_res").unwrap();
}
struct MeshSegIter<'a> {
    mesh: &'a TMFMesh,
    item: usize,
}
impl<'a> MeshSegIter<'a> {
    fn tmf_segs(mesh: &'a TMFMesh) -> Self {
        Self { mesh, item: 0 }
    }
}
impl<'a> std::iter::Iterator for MeshSegIter<'a> {
    type Item = DecodedSegment;
    fn next(&mut self) -> Option<Self::Item> {
        self.item += 1;
        match self.item {
            0 => panic!("Impossible condition reached."),
            1 => match self.mesh.get_vertices() {
                Some(vertices) => Some(DecodedSegment::AppendVertex(vertices.into())),
                None => self.next(),
            },
            2 => match self.mesh.get_normals() {
                Some(normals) => Some(DecodedSegment::AppendNormal(normals.into())),
                None => self.next(),
            },
            3 => match self.mesh.get_uvs() {
                Some(uvs) => Some(DecodedSegment::AppendUV(uvs.into())),
                None => self.next(),
            },
            4 => match self.mesh.get_vertex_triangles() {
                Some(tris) => Some(DecodedSegment::AppendTriangleVertex(tris.into())),
                None => self.next(),
            },
            5 => match self.mesh.get_normal_triangles() {
                Some(tris) => Some(DecodedSegment::AppendTriangleNormal(tris.into())),
                None => self.next(),
            },
            6 => match self.mesh.get_uv_triangles() {
                Some(tris) => Some(DecodedSegment::AppendTriangleUV(tris.into())),
                None => self.next(),
            },
            7 => match self.mesh.get_tangents() {
                Some(tans) => Some(DecodedSegment::AppendTangent(tans.into())),
                None => self.next(),
            },
            8 => match self.mesh.get_tangent_triangles() {
                Some(tans) => Some(DecodedSegment::AppendTriangleTangent(tans.into())),
                None => self.next(),
            },
            9..=usize::MAX => {
                let index = self.item - 9;
                let seg = self.mesh.custom_data.get(index)?;
                Some(DecodedSegment::AppendCustom(seg.clone()))
            }
            //Should never happen.
            _ => todo!(),
        }
    }
}
#[test]
#[cfg(feature = "obj_import")]
fn rw_susan_tmf() {
    init_test_env();
    let mut file = std::fs::File::open("testing/susan.obj").unwrap();
    let (tmf_mesh, name) = TMFMesh::read_from_obj_one(&mut file).unwrap();
    tmf_mesh.verify().unwrap();
    assert!(name == "Suzanne", "Name should be Suzanne but is {name}");
    let prec = TMFPrecisionInfo::default();
    let mut out = Vec::new();
    {
        futures::executor::block_on(write_tmf(&[(tmf_mesh, name)], &mut out, &prec)).unwrap();
    }
    let (r_mesh, name) = TMFMesh::read_tmf_one(&mut (&out as &[u8])).unwrap();
    assert!(name == "Suzanne", "Name should be Suzanne but is {name}");
    r_mesh.verify().unwrap();
}
