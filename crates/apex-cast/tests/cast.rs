use apex_cast::*;

/// Minimal Cast writer for synthetic test files.
enum P<'a> {
    S(&'a str),
    B(&'a [u8]),
    H(&'a [u16]),
    I(&'a [u32]),
    L(&'a [u64]),
    F(&'a [f32]),
    V2(&'a [[f32; 2]]),
    V3(&'a [[f32; 3]]),
    V4(&'a [[f32; 4]]),
}

struct N<'a> {
    kind: &'a [u8; 4],
    hash: u64,
    props: Vec<(&'a str, P<'a>)>,
    children: Vec<N<'a>>,
}

fn node(
    kind: &[u8; 4],
    hash: u64,
    props: Vec<(&'static str, P<'static>)>,
    children: Vec<N<'static>>,
) -> N<'static> {
    let kind: &'static [u8; 4] = Box::leak(Box::new(*kind));
    N {
        kind,
        hash,
        props,
        children,
    }
}

fn write_node(n: &N, out: &mut Vec<u8>) {
    let start = out.len();
    out.extend_from_slice(n.kind);
    out.extend_from_slice(&0u32.to_le_bytes()); // size, patched below
    out.extend_from_slice(&n.hash.to_le_bytes());
    out.extend_from_slice(&(n.props.len() as u32).to_le_bytes());
    out.extend_from_slice(&(n.children.len() as u32).to_le_bytes());
    for (name, p) in &n.props {
        let (ty, count, data): (&[u8; 2], usize, Vec<u8>) = match p {
            P::S(s) => (b"s\0", 1, [s.as_bytes(), &[0]].concat()),
            P::B(v) => (b"b\0", v.len(), v.to_vec()),
            P::H(v) => (
                b"h\0",
                v.len(),
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            P::I(v) => (
                b"i\0",
                v.len(),
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            P::L(v) => (
                b"l\0",
                v.len(),
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            P::F(v) => (
                b"f\0",
                v.len(),
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            P::V2(v) => (
                b"2v",
                v.len(),
                v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            P::V3(v) => (
                b"3v",
                v.len(),
                v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            P::V4(v) => (
                b"4v",
                v.len(),
                v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
            ),
        };
        out.extend_from_slice(ty);
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&(count as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&data);
    }
    for c in &n.children {
        write_node(c, out);
    }
    let size = (out.len() - start) as u32;
    out[start + 4..start + 8].copy_from_slice(&size.to_le_bytes());
}

fn file(root_children: Vec<N<'static>>) -> Vec<u8> {
    let root = node(b"root", 0, vec![], root_children);
    let mut out = b"cast".to_vec();
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    write_node(&root, &mut out);
    out
}

/// Two bones: root at the origin, "tip" 10 up the Y axis. One triangle on
/// the tip bone, one material with an albedo texture.
fn two_bone_model() -> Vec<u8> {
    let skel = node(
        b"skel",
        0,
        vec![],
        vec![
            node(
                b"bone",
                0,
                vec![
                    ("n", P::S("root")),
                    ("p", P::I(&[u32::MAX])),
                    ("lp", P::V3(&[[0.0; 3]])),
                    ("lr", P::V4(&[[0.0, 0.0, 0.0, 1.0]])),
                ],
                vec![],
            ),
            node(
                b"bone",
                0,
                vec![
                    ("n", P::S("tip")),
                    ("p", P::I(&[0])),
                    ("lp", P::V3(&[[0.0, 10.0, 0.0]])),
                    ("lr", P::V4(&[[0.0, 0.0, 0.0, 1.0]])),
                ],
                vec![],
            ),
        ],
    );
    let matl = node(
        b"matl",
        77,
        vec![
            ("n", P::S("gun")),
            ("t", P::S("pbr")),
            ("albedo", P::L(&[5])),
        ],
        vec![node(
            b"file",
            5,
            vec![("p", P::S("gun/gun_albedo.png"))],
            vec![],
        )],
    );
    let mesh = node(
        b"mesh",
        0,
        vec![
            ("n", P::S("body")),
            (
                "vp",
                P::V3(&[[1.0, 10.0, 0.0], [0.0, 11.0, 0.0], [0.0, 10.0, 1.0]]),
            ),
            (
                "vn",
                P::V3(&[[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]]),
            ),
            ("u0", P::V2(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])),
            ("mi", P::H(&[1])),
            ("wb", P::B(&[1, 1, 1])),
            ("wv", P::F(&[1.0, 1.0, 1.0])),
            ("f", P::H(&[0, 1, 2])),
            ("m", P::L(&[77])),
        ],
        vec![],
    );
    file(vec![node(b"modl", 0, vec![], vec![skel, matl, mesh])])
}

/// Rotates "tip" 90° about Z over 10 frames at 30 fps; one notetrack.
fn bend_anim() -> Vec<u8> {
    const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let curve = node(
        b"curv",
        0,
        vec![
            ("nn", P::S("tip")),
            ("kp", P::S("rq")),
            ("kb", P::B(&[0, 10])),
            ("kv", P::V4(&[[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, S, S]])),
            ("m", P::S("absolute")),
        ],
        vec![],
    );
    let note = node(
        b"nott",
        0,
        vec![("n", P::S("mag_in")), ("kb", P::B(&[5]))],
        vec![],
    );
    file(vec![node(
        b"anim",
        0,
        vec![("fr", P::F(&[30.0])), ("lo", P::B(&[1]))],
        vec![curve, note],
    )])
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

#[test]
fn reads_skeleton_mesh_and_material() {
    let m = Model::from_bytes(&two_bone_model()).unwrap();
    assert_eq!(m.skeleton.bones.len(), 2);
    assert_eq!(m.skeleton.bones[0].parent, None);
    assert_eq!(m.skeleton.bones[1].parent, Some(0));
    let mesh = &m.meshes[0];
    assert_eq!((mesh.positions.len(), mesh.indices.len()), (3, 3));
    assert_eq!(mesh.material, Some(0));
    assert_eq!(m.materials[0].texture("albedo"), Some("gun/gun_albedo.png"));
    let g = m.skeleton.globals(&m.skeleton.rest_locals());
    assert!(close(g[1].pos, [0.0, 10.0, 0.0]));
}

#[test]
fn animation_samples_and_skins() {
    let m = Model::from_bytes(&two_bone_model()).unwrap();
    let a = Animation::from_bytes(&bend_anim()).unwrap();
    assert!(a.looping && (a.duration() - 10.0 / 30.0).abs() < 1e-6);
    let rest = m.skeleton.rest_locals();
    let rest_g = m.skeleton.globals(&rest);
    let bind = a.bind(&m.skeleton);
    assert_eq!(bind, vec![Some(1)]);

    // At the end the tip bone is turned 90° about Z: a vertex 1 along +X
    // from the tip ends up 1 along +Y from it.
    let mut pose = rest.clone();
    a.sample_into(a.duration(), &bind, &rest, &mut pose, 1.0);
    let skin = skinning(&m.skeleton, &rest_g, &pose);
    let (mut p, mut n) = (Vec::new(), Vec::new());
    skin_mesh(&m.meshes[0], &skin, &[0, 1], &mut p, &mut n);
    assert!(close(p[0], [0.0, 11.0, 0.0]), "{:?}", p[0]);

    // Rest pose skins to the stored positions.
    let skin = skinning(&m.skeleton, &rest_g, &rest);
    skin_mesh(&m.meshes[0], &skin, &[0, 1], &mut p, &mut n);
    assert!(close(p[0], [1.0, 10.0, 0.0]));

    // Halfway is 45°.
    let mut half = rest.clone();
    a.sample_into(a.duration() / 2.0, &bind, &rest, &mut half, 1.0);
    let angle = 2.0 * half[1].rot[2].atan2(half[1].rot[3]);
    assert!(
        (angle.to_degrees() - 45.0).abs() < 0.5,
        "{}",
        angle.to_degrees()
    );

    let notes: Vec<_> = a.notes_between(0.1, 0.2).collect();
    assert_eq!(notes, ["mag_in"]);
}

#[test]
fn rejects_garbage() {
    assert_eq!(Model::from_bytes(b"nope").unwrap_err(), CastError::NotCast);
    let mut f = two_bone_model();
    f.truncate(f.len() - 10);
    assert!(Model::from_bytes(&f).is_err());
}

/// Real RSX exports, when present on this machine (never in the repo).
#[test]
fn personal_exports_load_when_present() {
    let gun =
        std::path::Path::new(r"F:\ApexContent\weapons\r99\mdl\Weapons\r97\ptpov_r97_LOD0.cast");
    let arms = std::path::Path::new(
        r"F:\ApexContent\characters\wattson\mdl\Weapons\arms\pov_pilot_light_wattson_LOD0.cast",
    );
    let reload = std::path::Path::new(
        r"F:\ApexContent\weapons\r99\animseq\weapons\r97\ptpov_r97\reload_seq_0.cast",
    );
    if !(gun.exists() && arms.exists() && reload.exists()) {
        eprintln!("personal Apex exports not present; skipped");
        return;
    }
    let gun = Model::from_bytes(&std::fs::read(gun).unwrap()).unwrap();
    let arms = Model::from_bytes(&std::fs::read(arms).unwrap()).unwrap();
    let reload = Animation::from_bytes(&std::fs::read(reload).unwrap()).unwrap();
    assert_eq!(gun.skeleton.bones.len(), 114);
    assert!(gun.meshes.len() > 10 && gun.materials.len() > 10);
    // Every arm bone exists in the gun skeleton, so gun animations drive it.
    assert!(
        arms.skeleton
            .bones
            .iter()
            .all(|b| gun.skeleton.index_of(&b.name).is_some())
    );
    let bind = reload.bind(&gun.skeleton);
    assert!(bind.iter().filter(|b| b.is_some()).count() > 100);
    assert!(reload.duration() > 1.0, "{}", reload.duration());

    let rest = gun.skeleton.rest_locals();
    let g = gun.skeleton.globals(&rest);
    let at = |n: &str| g[gun.skeleton.index_of(n).unwrap()].pos;
    eprintln!(
        "muzzle {:?} camera {:?} pov {:?}",
        at("muzzle_flash"),
        at("jx_c_camera"),
        at("jx_c_pov")
    );
    let mut pose = rest.clone();
    reload.sample_into(reload.duration() * 0.5, &bind, &rest, &mut pose, 1.0);
    let g2 = gun.skeleton.globals(&pose);
    let mag = gun.skeleton.index_of("def_c_magazine").unwrap();
    eprintln!(
        "magazine rest {:?} mid-reload {:?}",
        g[mag].pos, g2[mag].pos
    );
}
