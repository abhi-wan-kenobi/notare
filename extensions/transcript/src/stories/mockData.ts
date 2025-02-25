import type { TimelineViewItem } from "@hypr/plugin-listener";

export const mockTimeline = {
  items: [
    {
      start: 3,
      end: 8,
      speaker: "John",
      text: "Welcome everyone to our product planning meeting. Let's review our Q1 progress and discuss the roadmap for Q2.",
    },
    {
      start: 10,
      end: 15,
      speaker: "Sarah",
      text: "Thanks John. Before we dive in, should we wait for Mike? He mentioned he might be a minute late.",
    },
    {
      start: 17,
      end: 20,
      speaker: "John",
      text: "Yes, let's give him a couple more minutes.",
    },
    {
      start: 45,
      end: 48,
      speaker: "Mike",
      text: "Sorry I'm late everyone! Had some technical issues.",
    },
    {
      start: 50,
      end: 55,
      speaker: "John",
      text: "No problem Mike. Sarah, would you like to start with the Q1 review?",
    },
    {
      start: 58,
      end: 85,
      speaker: "Sarah",
      text: "Of course. In Q1, we successfully launched the new dashboard interface and improved our API response times by 40%. The user feedback has been overwhelmingly positive, with our NPS score increasing from 45 to 62.",
    },
    {
      start: 88,
      end: 95,
      speaker: "Mike",
      text: "Those are impressive numbers. How did the mobile app redesign perform?",
    },
    {
      start: 98,
      end: 120,
      speaker: "Sarah",
      text: "The mobile app redesign exceeded expectations. We saw a 25% increase in daily active users and the average session duration went up by 3 minutes. The new gesture controls were particularly well-received.",
    },
    {
      start: 123,
      end: 140,
      speaker: "John",
      text: "Great work team. For Q2, I think we should focus on the AI-powered features we discussed last month. Mike, could you share the technical feasibility assessment?",
    },
    {
      start: 142,
      end: 180,
      speaker: "Mike",
      text: "Absolutely. We've completed the initial assessment and identified three key features we can implement in Q2: smart categorization, automated summarization, and predictive analytics. The infrastructure work would take about 3 weeks, and then we can roll out features incrementally.",
    },
    {
      start: 183,
      end: 195,
      speaker: "Sarah",
      text: "That timeline works well with our user research schedule. We can start testing with our beta users by mid-quarter.",
    },
    {
      start: 198,
      end: 210,
      speaker: "John",
      text: "Perfect. Let's break down the specific deliverables and set some milestones. Mike, can you share the technical architecture diagram?",
    },
  ] satisfies TimelineViewItem[],
};
