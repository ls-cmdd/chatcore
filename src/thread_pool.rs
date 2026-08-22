//! الطبقة 4: Thread Pool يدوي.
//! بدل ما نفتح thread جديد لكل اتصال (مكلف مع زيادة الحمل)، بنفتح عدد ثابت
//! من الـ workers من البداية، وأي شغل جديد بيتحط في طابور (queue) عبر mpsc channel،
//! والـ workers بياخدوا منه شغل وقت ما يفضوا.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// ينشئ thread pool بعدد ثابت من الـ workers.
    /// size = 0 غير مسموح لأنه معناه السيرفر مش هيقدر يعالج أي طلب.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0, "عدد الـ workers لازم يكون أكبر من صفر");

        let (sender, receiver) = mpsc::channel::<Job>();
        // الـ Receiver مش قابل للـ clone، فبنحطه جوه Arc<Mutex<>> عشان
        // كل الـ workers يقدروا ياخدوا شغل من نفس الطابور بأمان (mutual exclusion).
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// يضيف شغل جديد (closure) لطابور الـ pool. أول worker فاضي هياخده وينفذه.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        if let Some(sender) = &self.sender {
            // لو الإرسال فشل (نادرًا جدًا، فقط لو كل الـ workers ماتوا) بنتجاهل
            // بدل ما نـ panic السيرفر كله.
            let _ = sender.send(job);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // بنقفل الـ sender الأساسي عشان الـ workers يعرفوا مفيش شغل جديد جاي
        // (recv() هيرجع Err لما كل الـ senders يتقفلوا) فيخرجوا من الحلقة بأمان.
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

struct Worker {
    #[allow(dead_code)]
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            // بنقفل الطابور، ناخد شغلانة واحدة بس، وبعدين نفك القفل فورًا
            // (عشان worker تاني يقدر ياخد شغل تاني وإحنا لسه بنعالج شغلتنا).
            let job = receiver.lock().unwrap().recv();
            match job {
                Ok(job) => job(),
                Err(_) => break, // الطابور اتقفل نهائيًا -> اخرج من الحلقة
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
