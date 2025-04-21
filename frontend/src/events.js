const ANALYTICS_ENDPOINT = "/api/event";

export const trackEvent = async (userId, eventType, action, info = {}) => {
  if (!userId) {
    console.warn("Analytics: Missing userId, event not sent.");
    return;
  }

  const eventPayload = {
    user_id: Number(userId), 
    event_type: eventType,
    action: action,
    info: info,
    
  };

  console.log("Sending Analytics Event:", eventPayload);

  try {
    const response = await fetch(ANALYTICS_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(eventPayload),
      
    });

    if (!response.ok) {
      console.error(
        `Analytics Error: Failed to send event. Status: ${response.status}`
      );
      
      const errorBody = await response.text();
      console.error('Analytics Error Body:', errorBody);
    } else {
      console.log('Analytics: Event sent successfully');
    }
  } catch (error) {
    console.error("Analytics Error: Network or other error sending event:", error);
  }
};


export const getOrGenerateUserId = () => {
    const STORAGE_KEY = 'demo_user_id';
    let userId = localStorage.getItem(STORAGE_KEY);
    if (!userId) {
        userId = `${Date.now()}-${Math.floor(Math.random() * 1e9)}`;
        localStorage.setItem(STORAGE_KEY, userId);
    }
    try {
      return Number(userId.split('-')[0]);
    } catch(e) {
      console.error("Failed to parse userId timestamp as Number", e);
      return Date.now();
    }
};