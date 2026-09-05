export function formattedDate(date: Date) {
    return date.toLocaleDateString("en-US", {
        year: "numeric",
        month: "long",
        day: "numeric",
    });
}

export function formattedDateTime(date: Date | null) {
    if (!date || date.getTime() === 0) {
        return "None";
    }
    return date.toLocaleString("en-US", {
        year: "numeric",
        month: "long",
        day: "numeric",
        hour: "numeric",
        minute: "numeric",
    });
}

export function dateFromUnixSeconds(value: number): Date {
    return new Date(value * 1000);
}

export function formattedUnixDateTime(value: number | null): string {
    return value === null ? "None" : formattedDateTime(dateFromUnixSeconds(value));
}
