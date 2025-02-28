import { Card } from "@hypr/ui/components/ui/card";
import { Badge } from "@hypr/ui/components/ui/badge";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@hypr/ui/components/ui/avatar";
import type { Ticket } from "../../type";
import { PriorityIcons } from "./priority";
import { StatusIcons } from "./status";

export default function TicketCard({ ticket }: { ticket: Ticket }) {
  const PriorityIcon = (() => {
    switch (ticket.priority) {
      case "Urgent":
        return PriorityIcons.Urgent;
      case "High":
        return PriorityIcons.High;
      case "Medium":
        return PriorityIcons.Medium;
      case "Low":
        return PriorityIcons.Low;
      case "NoPriority":
      default:
        return PriorityIcons.NoPriority;
    }
  })();

  const StatusIcon = (() => {
    switch (ticket.status) {
      case "Backlog":
        return StatusIcons.Backlog;
      case "To Do":
        return StatusIcons["To Do"];
      case "In Progress":
        return StatusIcons["In Progress"];
      case "Done":
        return StatusIcons.Done;
      case "Canceled":
        return StatusIcons.Canceled;
      case "Duplicate":
        return StatusIcons.Duplicate;
      default:
        return StatusIcons.Backlog;
    }
  })();

  const getInitials = (name: string) => {
    if (name === "Unassigned") return "UN";
    return name
      .split(" ")
      .map((part) => part[0])
      .join("")
      .toUpperCase();
  };

  return (
    <Card className="p-4 hover:shadow-md transition-all cursor-pointer">
      <div className="flex flex-col gap-2">
        {ticket.id && (
          <div className="w-fit text-xs font-mono text-gray-500 bg-gray-100 px-1 py-0.5 rounded">
            {ticket.id}
          </div>
        )}

        <div className="flex items-center gap-2">
          <h3 className="font-medium text-sm line-clamp-1">{ticket.title}</h3>
        </div>

        <p className="text-xs text-gray-600 line-clamp-2">
          {ticket.description}
        </p>

        <div className="flex items-center justify-between mt-2">
          <div className="flex items-center gap-2">
            <Badge
              variant="outline"
              className="flex items-center gap-1 text-gray-600 border px-2 py-1.5"
            >
              <StatusIcon />
              <span className="text-xs">{ticket.status}</span>
            </Badge>

            <Badge
              variant="outline"
              className="flex items-center gap-1 text-gray-600 border px-2 py-1.5"
            >
              <PriorityIcon />
              <span className="text-xs">{ticket.priority}</span>
            </Badge>
          </div>

          <Avatar className="h-6 w-6">
            <AvatarImage
              src={`https://avatar.vercel.sh/${ticket.assignee}.png`}
              alt={ticket.assignee}
            />
            <AvatarFallback className="text-xs">
              {getInitials(ticket.assignee)}
            </AvatarFallback>
          </Avatar>
        </div>
      </div>
    </Card>
  );
}
