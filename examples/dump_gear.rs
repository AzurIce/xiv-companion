fn main() {
    let p = std::env::args().nth(1).expect("GEARSET.DAT");
    let b = std::fs::read(p).unwrap();
    let g = physis::savedata::gearsets::GearSets::from_existing(&b).expect("parse gearsets");
    println!("current={}", g.current_gearset);
    for (i, x) in g.gearsets.iter().enumerate() { if let Some(x)=x { println!("{} {:?}", i, x); } }
}
