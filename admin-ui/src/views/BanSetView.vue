<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref, watch } from 'vue';
import { useRoute } from 'vue-router';

const route = useRoute();

interface StickerPub {
  id: string,
  set_id: string,
}

const url = computed(() => `http://localhost:3002/api/sets/${route.params.setId}/stickers`)

const { data, error, execute: refetch } = useFetch<StickerPub[]>(url, { refetch: true, updateDataOnError: true }).json()


watch(error, () => console.log(error))

const isBanned = ref(false)
const isApproved = ref(false)

watch(url, () => {
  isBanned.value = false;
  isApproved.value = false;
})

const approve = async () => {
    const { data, error } = await useFetch(`http://localhost:3002/api/sets/${route.params.setId}/approve`).post()
    console.log(data, error)
    if (error.value) {
      alert(error.value)
    }
  }

const toggleBan = async () => {
  if (isBanned.value) {
    const { data, error } = await useFetch(`http://localhost:3002/api/sets/${route.params.setId}/unban`).post()
    console.log(data, error)
    if (!error.value) {
      isBanned.value = false;
    }
  } else {
    const { data, error } = await useFetch(`http://localhost:3002/api/sets/${route.params.setId}/ban`).post()
    console.log(data, error)
    if (!error.value) {
      isBanned.value = true;
    }
  }
}

</script>

<template>
  <div class="d-flex">
    <div>
    <h1>This is an about page</h1>
    {{ route.params.setId }}
    asdf
    {{ error }}

    isBanned: {{ isBanned }}

    <v-btn @click="toggleBan" v-if="isBanned">
      Unban
    </v-btn>
    <v-btn @click="toggleBan" v-else color="error">
      Ban
    </v-btn>
    <v-btn @click="approve" color="success">
      Approve
    </v-btn>

    <div v-if="data">
      <div v-for="sticker of data">
        <img :src="`http://localhost:3001/files/stickers/${sticker.id}/thumbnail.png`" loading="lazy" width="128"
          height="128" />
        {{ sticker }}
        <v-btn :to="{ name: 'banSimilarView', params: { stickerId: sticker.id } }">
          similar view
        </v-btn>
      </div>
    </div>
</div>
<div>
    <router-view />
</div>
  </div>

</template>

<style></style>
