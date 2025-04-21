
import { useState, useEffect, useRef, useCallback } from "react";
import { trackEvent, getOrGenerateUserId } from "./events"; 
import "./App.css"; 

const API_URL = "https://jsonplaceholder.typicode.com/posts";
const POSTS_PER_PAGE = 15; 

function App() {
  const [userId, setUserId] = useState(null);
  const [posts, setPosts] = useState([]);
  const [page, setPage] = useState(1); 
  const [loading, setLoading] = useState(false);
  const [hasMore, setHasMore] = useState(true); 
  const [error, setError] = useState(null);
  const [scrollPageCount, setScrollPageCount] = useState(0); 

  const observer = useRef(); 

  const fetchPosts = useCallback(async (pageNum) => {
    if (loading || !hasMore) return; 

    setLoading(true);
    setError(null);
    console.log(`Fetching page ${pageNum}...`);

    try {
      const response = await fetch(
        `${API_URL}?_page=${pageNum}&_limit=${POSTS_PER_PAGE}`
      );
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const newPosts = await response.json();

      setPosts((prevPosts) => [...prevPosts, ...newPosts]);
      setPage(pageNum); 
      setHasMore(newPosts.length === POSTS_PER_PAGE); 

      if (pageNum > 1) {
        
        const newScrollCount = pageNum -1; 
        setScrollPageCount(newScrollCount);
        
        if (userId) {
            trackEvent(userId, "scroll", "infinite_scroll_page", {
              page_loaded: newScrollCount,
            });
        }
      }
    } catch (e) {
      console.error("Failed to fetch posts:", e);
      setError("Failed to load posts. Please try refreshing.");
      setHasMore(false); 
    } finally {
      setLoading(false);
    }
  }, [loading, hasMore, userId]); 


  
  useEffect(() => {
    const generatedUserId = getOrGenerateUserId();
    setUserId(generatedUserId);
    trackEvent(generatedUserId, "page_view", "load", { path: "/" });

    
    fetchPosts(1);
     
  }, []); 

  const lastPostElementRef = useCallback(node => {
    if (loading) return; 
    if (observer.current) observer.current.disconnect(); 

    observer.current = new IntersectionObserver(entries => {
      
      if (entries[0].isIntersecting && hasMore) {
        console.log('Last element visible, loading more...');
        fetchPosts(page + 1); 
      }
    });

    if (node) observer.current.observe(node); 
  }, [loading, hasMore, page, fetchPosts]); 

  const handlePostClick = (postId) => {
    console.log(`Post clicked: ${postId}`);
    if (userId) {
      trackEvent(userId, "click", "post_item", { post_id: postId });
    }
  };

  return (
    <div className="App">
      <h1>Infinite Scroll Posts</h1>
      <p>Scroll down to load more...</p>
      <p>Infinite Scroll Pages Loaded: {scrollPageCount}</p>
      <div className="post-list">
        {posts.map((post, index) => {
          
          if (posts.length === index + 1) {
             return (
                <div
                  ref={lastPostElementRef}
                  key={post.id}
                  className="post-item"
                  onClick={() => handlePostClick(post.id)}
                >
                  <h2>({post.id}) {post.title}</h2>
                  <p>{post.body}</p>
                </div>
             )
          } else {
             return (
                <div
                  key={post.id}
                  className="post-item"
                  onClick={() => handlePostClick(post.id)}
                >
                  <h2>({post.id}) {post.title}</h2>
                  <p>{post.body}</p>
                </div>
             )
          }
        })}
      </div>
      {loading && <p className="status">Loading more posts...</p>}
      {!hasMore && posts.length > 0 && <p className="status">You've reached the end!</p>}
      {error && <p className="status error">{error}</p>}
    </div>
  );
}

export default App;