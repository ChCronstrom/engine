use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread;

use crate::search::Searcher;


pub struct SearchInterface
{
    join_handle: Option<thread::JoinHandle<()>>, // Actually always Some, except during drop
    stop_conditions: Box<StopConditions>,
    channel: mpsc::Sender<ThreadCommand>,
}

impl SearchInterface
{
    pub fn new() -> Self
    {
        let stop_conditions = Box::new(StopConditions::new());
        // SAFETY: Artificially prolonging the lifetime of the borrow. This is sound so long as I make
        // sure to join with the thread before dropping the box.
        let borrowed_stop_conditions = unsafe { &*(&*stop_conditions as *const _) };
        let (sender, receiver) = mpsc::channel();
        let join_handle = thread::Builder::new()
            .name("search thread".into())
            .spawn(move || search_thread_mainloop(receiver, borrowed_stop_conditions))
            .expect("failed to start thread");

        SearchInterface {
            stop_conditions,
            join_handle: Some(join_handle),
            channel: sender,
        }
    }

    pub fn go(&mut self, position: &chess::Board, stop_conditions: StopConditions)
    {
        // If search is running, get it to stop
        if self.is_running()
        {
            self.set_stop_now(true);

            // Wait for it to stop
            while self.is_running()
            {
                std::hint::spin_loop();
                std::thread::yield_now();
            }
        }

        // Set new stop parameters
        self.set_stop_now(false);
        self.stop_conditions.assign(stop_conditions);
        
        // Give new position to thread
        self.channel.send(ThreadCommand::Go(*position))
            .expect("channel mustn't close");
    }

    pub fn stop(&mut self)
    {
        self.set_stop_now(true);
    }

    fn is_running(&mut self) -> bool
    {
        self.stop_conditions.is_running.load(Ordering::Acquire)
    }

    fn set_stop_now(&mut self, value: bool)
    {
        self.stop_conditions.stop_now.store(value, Ordering::Release);
    }
}

impl Drop for SearchInterface
{
    fn drop(&mut self)
    {
        // Stop search on thread, if ongoing
        self.set_stop_now(true);

        // Ask thread to exit
        self.channel.send(ThreadCommand::Exit).expect("channel mustn't close");

        // Wait for thread to terminate
        let join_handle = self.join_handle.take().expect("no join handle?");
        join_handle.join().expect("search thread panic");

        // We can now proceed with dropping the channel and stop conditions box
    }
}

fn search_thread_mainloop(channel: mpsc::Receiver<ThreadCommand>, stop_conditions: &StopConditions)
{
    let mut searcher = Searcher::new(stop_conditions);
    loop {
        match channel.recv().expect("channel mustn't close") {
            ThreadCommand::Go(position) => searcher.search(position),
            ThreadCommand::Exit => break,
        }
    }
}

enum ThreadCommand
{
    Go(chess::Board),
    Exit,
}

pub struct StopConditions
{
    pub is_running: AtomicBool,
    pub stop_now: AtomicBool,
    pub depth: AtomicU8,
}

impl StopConditions
{
    pub fn new() -> Self
    {
        StopConditions {
            stop_now: AtomicBool::new(false),
            is_running: AtomicBool::new(false),
            depth: AtomicU8::new(255),
        }
    }

    pub fn assign(&self, new: Self)
    {
        self.depth.store(new.depth.into_inner(), Ordering::Release);
    }
}
