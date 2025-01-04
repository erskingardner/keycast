import { json } from "@sveltejs/kit";

export const GET = () => {
    return json({
        status: "ok",
        timestamp: new Date().toISOString(),
        service: "web",
    });
};
