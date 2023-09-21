use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};

use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::thread::ThreadId;
lazy_static! {
    pub static ref MANAGER_VARIABLE: Mutex<LockManager> = Mutex::new(LockManager::new());
}
type LockId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum LockType {
    Mutex,
    RMutex,
    WMutex,
    CondVar,
    Channel,
}
#[derive(Clone, PartialEq, Eq, Hash)]
enum DependencyType {
    Mutex,
    Signal,
    Send,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SLock {
    l_type: LockType,
    l_id: LockId,
}

#[derive(Clone, PartialEq, Eq)]
struct DependencyRelation {
    dep_type: DependencyType,
    t_id: ThreadId,
    l: SLock,
    lock_set: HashSet<SLock>,
}
impl Hash for DependencyRelation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for lock in &self.lock_set {
            lock.hash(state);
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
enum RsType {
    S(SLock),
    D(DependencyRelation),
}

pub struct LockManager {
    tid_to_lockset: HashMap<ThreadId, HashSet<SLock>>,
    tid_to_recent_status: HashMap<ThreadId, HashSet<RsType>>,
    tid_to_dependency_set: HashMap<ThreadId, HashSet<DependencyRelation>>,
}

impl LockManager {
    pub fn new() -> Self {
        LockManager {
            tid_to_lockset: HashMap::new(),
            tid_to_recent_status: HashMap::new(),
            tid_to_dependency_set: HashMap::new(),
        }
    }

    fn mutex_lock(&mut self, t_id: ThreadId, m_id: LockId) {
        if self.tid_to_lockset.get(&t_id).unwrap().is_empty() {
            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::S(SLock {
                    l_type: LockType::Mutex,
                    l_id: m_id,
                }));
        } else {
            let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::D(DependencyRelation {
                    dep_type: DependencyType::Mutex,
                    t_id: t_id,
                    l: SLock {
                        l_type: LockType::Mutex,
                        l_id: m_id,
                    },
                    lock_set: ls,
                }));
        }
        self.tid_to_lockset.get_mut(&t_id).unwrap().insert(SLock {
            l_type: LockType::Mutex,
            l_id: m_id,
        });
    }

    fn mutex_unlock(&mut self, t_id: ThreadId, m_id: LockId) {
        let mut ls = self.tid_to_lockset.get_mut(&t_id).unwrap();

        ls.remove(&SLock {
            l_type: LockType::Mutex,
            l_id: m_id,
        });
    }

    fn rwmutex_rlock(&mut self, t_id: ThreadId, m_id: LockId) {
        if self.tid_to_lockset.get(&t_id).unwrap().is_empty() {
            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::S(SLock {
                    l_type: LockType::RMutex,
                    l_id: m_id,
                }));
        } else {
            let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();
            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::D(DependencyRelation {
                    dep_type: DependencyType::Mutex,
                    t_id: t_id,
                    l: SLock {
                        l_type: LockType::RMutex,
                        l_id: m_id,
                    },
                    lock_set: ls,
                }));
        }
    }

    fn rwmutex_runlock(&mut self, t_id: ThreadId, m_id: LockId) {
        let ls = self.tid_to_lockset.get_mut(&t_id).unwrap();

        ls.remove(&SLock {
            l_type: LockType::RMutex,
            l_id: m_id,
        });
    }

    fn rwmutex_wlock(&mut self, t_id: ThreadId, m_id: LockId) {
        if self.tid_to_lockset.get(&t_id).unwrap().is_empty() {
            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::S(SLock {
                    l_type: LockType::WMutex,
                    l_id: m_id,
                }));
        } else {
            let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();
            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::D(DependencyRelation {
                    dep_type: DependencyType::Mutex,
                    t_id: t_id,
                    l: SLock {
                        l_type: LockType::WMutex,
                        l_id: m_id,
                    },
                    lock_set: ls,
                }));
        }
    }

    fn rwmutex_wunlock(&mut self, t_id: ThreadId, m_id: LockId) {
        let ls = self.tid_to_lockset.get_mut(&t_id).unwrap();

        ls.remove(&SLock {
            l_type: LockType::WMutex,
            l_id: m_id,
        });
    }

    fn cond_wait(&mut self, t_id: ThreadId, cv_id: LockId, assoc_id: LockId) {
        if self.tid_to_lockset.get(&t_id).unwrap().len() == 1 {
            for i in self.tid_to_lockset.get(&t_id).unwrap() {
                if *i
                    == (SLock {
                        l_type: LockType::Mutex,
                        l_id: assoc_id,
                    })
                {
                    self.tid_to_recent_status
                        .get_mut(&t_id)
                        .unwrap()
                        .insert(RsType::S(SLock {
                            l_type: LockType::CondVar,
                            l_id: cv_id,
                        }));
                }
            }
        } else {
            let locks = self.tid_to_lockset.get_mut(&t_id).unwrap();
            // for i in 0..locks.len() {
            //     if locks[i].l_type == LockType::Mutex && locks[i].l_id == assoc_id {
            //         locks.remove(i);
            //         break;
            //     }
            // }
            locks.remove(&SLock {
                l_type: LockType::Mutex,
                l_id: assoc_id,
            });

            let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();
            self.tid_to_recent_status
                .get_mut(&t_id)
                .unwrap()
                .insert(RsType::D(DependencyRelation {
                    dep_type: DependencyType::Signal,
                    t_id: t_id,
                    l: SLock {
                        l_type: LockType::CondVar,
                        l_id: cv_id,
                    },
                    lock_set: ls,
                }));
        }
    }

    fn cond_notify(&mut self, t_id: ThreadId, cv_id: LockId) {
        let mut rs_new: HashSet<RsType> = HashSet::new();
        let mut rs_temp: HashSet<RsType> = HashSet::new();
        let rs = self.tid_to_recent_status.get_mut(&t_id).unwrap();
        for i in rs.iter() {
            match i {
                RsType::D(d) => {
                    let mut temp_d = d.clone();
                    temp_d.lock_set.insert(SLock {
                        l_type: LockType::CondVar,
                        l_id: cv_id,
                    });
                    rs_temp.insert(RsType::D(temp_d));
                }
                RsType::S(l) => match l.l_type {
                    LockType::Mutex | LockType::RMutex | LockType::WMutex => {
                        let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

                        rs_new.insert(RsType::D(DependencyRelation {
                            dep_type: DependencyType::Mutex,
                            t_id: t_id, 
                            l: SLock {
                                l_type: LockType::CondVar,
                                l_id: cv_id,
                            },
                            lock_set: ls,
                        }));
                    }
                    LockType::CondVar => {
                        let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

                        rs_new.insert(RsType::D(DependencyRelation {
                            dep_type: DependencyType::Signal,
                            t_id: t_id,
                            l: SLock {
                                l_type: LockType::CondVar,
                                l_id: cv_id,
                            },
                            lock_set: ls,
                        }));
                    }

                    _ => {
                        let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

                        rs_new.insert(RsType::D(DependencyRelation {
                            dep_type: DependencyType::Send,
                            t_id: t_id,
                            l: SLock {
                                l_type: LockType::CondVar,
                                l_id: cv_id,
                            },
                            lock_set: ls,
                        }));
                    }
                },
            }
        }

        let rss: HashSet<RsType> = rs_new.union(&rs_temp).cloned().collect();
        rs.clear();
        rs.extend(rss);
    }

    fn chan_recv(&mut self, t_id: ThreadId, ch_id: LockId) {
        let locks = self.tid_to_lockset.get(&t_id).unwrap().clone();
        self.tid_to_recent_status
            .get_mut(&t_id)
            .unwrap()
            .insert(RsType::D(DependencyRelation {
                dep_type: DependencyType::Send,
                t_id: t_id,
                l: SLock {
                    l_type: LockType::Channel,
                    l_id: ch_id,
                },
                lock_set: locks,
            }));
    }

    fn chan_send(&mut self, t_id: ThreadId, ch_id: LockId) {
        let mut rs_new: HashSet<RsType> = HashSet::new();
        let mut rs_temp: HashSet<RsType> = HashSet::new();
        let rs = self.tid_to_recent_status.get_mut(&t_id).unwrap();
        for i in rs.iter() {
            match i {
                RsType::D(d) => {
                    let mut temp_d = d.clone();
                    temp_d.lock_set.insert(SLock {
                        l_type: LockType::Channel,
                        l_id: ch_id,
                    });
                    rs_temp.insert(RsType::D(temp_d));
                }
                RsType::S(l) => match l.l_type {
                    LockType::Mutex | LockType::RMutex | LockType::WMutex => {
                        let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

                        rs_new.insert(RsType::D(DependencyRelation {
                            dep_type: DependencyType::Mutex,
                            t_id: t_id,
                            l: SLock {
                                l_type: LockType::Channel,
                                l_id: ch_id,
                            },
                            lock_set: ls,
                        }));
                    }
                    LockType::CondVar => {
                        let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

                        rs_new.insert(RsType::D(DependencyRelation {
                            dep_type: DependencyType::Signal,
                            t_id: t_id,
                            l: SLock {
                                l_type: LockType::Channel,
                                l_id: ch_id,
                            },
                            lock_set: ls,
                        }));
                    }

                    _ => {
                        let ls = self.tid_to_lockset.get(&t_id).unwrap().clone();

                        rs_new.insert(RsType::D(DependencyRelation {
                            dep_type: DependencyType::Send,
                            t_id: t_id,
                            l: SLock {
                                l_type: LockType::Channel,
                                l_id: ch_id,
                            },
                            lock_set: ls,
                        }));
                    }
                },
            }
        }

        let rss: HashSet<RsType> = rs_new.union(&rs_temp).cloned().collect();
        rs.clear();
        rs.extend(rss);
    }
}
