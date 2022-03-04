
// inside cellls
pub fn update_extents(&mut self) {
    let mut min_extents = ivec3(9999999, 9999999, 9999999);
    let mut max_extents = ivec3(-9999999, -9999999, -9999999);
    for cell in self.states.iter() {
        let pos = cell.0;
        if pos.x < min_extents.x {
            min_extents.x = pos.x;
        }
        if pos.y < min_extents.y {
            min_extents.y = pos.y;
        }
        if pos.z < min_extents.z {
            min_extents.z = pos.z;
        }
        if pos.x > max_extents.x {
            max_extents.x = pos.x;
        }
        if pos.y > max_extents.y {
            max_extents.y = pos.y;
        }
        if pos.z > max_extents.z {
            max_extents.z = pos.z;
        }
    }
    self.min_extents = min_extents;
    self.max_extents = max_extents;
}